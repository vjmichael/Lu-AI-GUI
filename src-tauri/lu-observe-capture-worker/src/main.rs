//! OBSERVE-WIN-CAPTURE-05: one-shot Windows.Graphics.Capture picker and
//! in-memory D3D11 frame readback, launched ONLY through Lu's existing
//! BackgroundProcessSupervisor after separate native user confirmation.
//!
//! Does not save screenshots, stream pixels to Lu/LLM, capture microphone,
//! persist window titles, enumerate arbitrary windows, or grant UI control.
//! The OS picker chooses exactly one window OR display. Current OS-pick
//! source type is not independently attested as a window identity; do not
//! bind its output to Lu's app-only observation scope or infer test PASS.
//! Task stdout follows the existing bounded P / D host receipt protocol.
#![deny(unsafe_code)]

const MAX_FRAME_WIDTH:u32=7680;
const MAX_FRAME_HEIGHT:u32=4320;
const MAX_FRAME_BYTES:usize=(MAX_FRAME_WIDTH as usize)*(MAX_FRAME_HEIGHT as usize)*4;
/// Validates the actual mapped D3D11 row layout. Accepts ordinary pitch
/// padding only within a fixed bounded allocation; rejects truncated frames.
fn frame_geometry_valid(width:u32,height:u32,row_pitch:u32,buffer_len:usize)->bool {
    if width==0||height==0||width>MAX_FRAME_WIDTH||height>MAX_FRAME_HEIGHT{return false;}
    let Some(row_bytes)=(width as usize).checked_mul(4) else{return false;};
    let Some(pitch)=usize::try_from(row_pitch).ok() else{return false;};
    if pitch<row_bytes||pitch>row_bytes.saturating_add(4096){return false;}
    let Some(mapped_len)=pitch.checked_mul(height as usize) else{return false;};
    let Some(useful_end)=pitch.checked_mul(height as usize-1)
        .and_then(|n|n.checked_add(row_bytes)) else{return false;};
    let max_padded=MAX_FRAME_BYTES+(MAX_FRAME_HEIGHT as usize)*4096;
    mapped_len<=max_padded&&buffer_len>=useful_end&&buffer_len<=max_padded
}
#[cfg(test)]
mod frame_geometry_tests {
    use super::*;
    #[test]
    fn accepts_mapped_bgra_frame_with_bounded_pitch_padding(){
        assert!(frame_geometry_valid(1920,1080,7680,7680*1080));
        assert!(frame_geometry_valid(1919,1080,7680,7680*1080));
    }
    #[test]
    fn rejects_invalid_dimensions_truncation_and_unbounded_pitch(){
        assert!(!frame_geometry_valid(0,1080,7680,7680*1080));
        assert!(!frame_geometry_valid(7681,1080,7681*4,7681*4*1080));
        assert!(!frame_geometry_valid(1920,1080,300,300*1080));
        assert!(!frame_geometry_valid(1920,1080,7680,100));
        assert!(!frame_geometry_valid(1,1,u32::MAX,usize::MAX));
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Lu Graphics Capture worker requires interactive Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
mod windows_capture_worker {
    // A child module does not implicitly import names defined in its parent.
    // Keep the native D3D11 callback and its geometry checks bound to the
    // same validated limits used by the host-independent unit tests.
    use super::{frame_geometry_valid, MAX_FRAME_WIDTH, MAX_FRAME_HEIGHT};
    use sha2::{Digest, Sha256};
    use std::{env,error::Error,io::{self,Write},time::{Instant,Duration}};
    use windows_capture::{
        capture::{Context,GraphicsCaptureApiHandler},
        frame::Frame,
        graphics_capture_api::InternalCaptureControl,
        graphics_capture_picker::GraphicsCapturePicker,
        window::Window,
        settings::{
            ColorFormat,CursorCaptureSettings,DirtyRegionSettings,
            DrawBorderSettings,MinimumUpdateIntervalSettings,
            SecondaryWindowSettings,Settings
        },
    };

    const MAX_CAPTURE_SECS:u64=20;

    struct OneFrame {
        started:Instant,
        captured:bool,
        nonce:String,
        foreground_identity:Option<(usize,u32)>,
    }
    impl GraphicsCaptureApiHandler for OneFrame {
        type Flags=(String,Option<(usize,u32)>);
        type Error=Box<dyn Error+Send+Sync>;
        fn new(ctx:Context<Self::Flags>)->Result<Self,Self::Error>{
            Ok(Self{started:Instant::now(),captured:false,
                nonce:ctx.flags.0,foreground_identity:ctx.flags.1})
        }
        fn on_frame_arrived(
            &mut self,frame:&mut Frame,
            control:InternalCaptureControl
        )->Result<(),Self::Error>{
            if self.captured || self.started.elapsed().as_secs()>MAX_CAPTURE_SECS {
                control.stop();
                return Ok(());
            }
            // Unlike the generic WGC OS picker, this foreground-window
            // mode binds to a native HWND + current owner PID, and fails
            // closed if that exact window is closed or replaced before
            // the only frame arrives. No window title is trusted.
            if let Some((hwnd,pid))=self.foreground_identity{
                let window=Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void);
                if !window.is_valid()||window.process_id()?!=pid{
                    control.stop();
                    return Err("Foreground window identity changed".into());
                }
            }
            let width=frame.width();
            let height=frame.height();
            if width==0||height==0
                ||width>MAX_FRAME_WIDTH||height>MAX_FRAME_HEIGHT{
                control.stop();
                return Err("Capture dimensions exceed Lu's bounded frame limit".into());
            }
            let mut gpu_frame=frame.buffer()?;
            let row_pitch=gpu_frame.row_pitch();
            let buffer=gpu_frame.as_raw_buffer();
            if !frame_geometry_valid(width,height,row_pitch,buffer.len()){
                control.stop();
                return Err("Invalid mapped D3D11 frame stride or frame size".into());
            }
            // Read actual WGC/D3D11-mapped bytes. No screenshot file, video
            // recording, user text, clipboard, window title or pixel IPC.
            // This hash is a *capture receipt*, not visual understanding.
            let mut sha=Sha256::new();
            sha.update(b"Lu-WGC-one-shot-frame-v1");
            sha.update(self.nonce.as_bytes());
            sha.update(width.to_le_bytes());
            sha.update(height.to_le_bytes());
            // Hash only active BGRA pixels. D3D11 row-pitch padding is not
            // image content and must not affect a privacy-preserving receipt.
            let pitch=row_pitch as usize;
            let active_bytes=(width as usize)*4;
            for row in 0..height as usize {
                let start=row*pitch;
                sha.update(&buffer[start..start+active_bytes]);
            }
            let digest=format!("{:x}",sha.finalize());
            // FrameBuffer release unmaps staging surface when dropped.
            // The operating system owns the GPU/driver buffers; Lu does not
            // retain or copy the pixel data into a persistent heap or file.
            drop(gpu_frame);
            self.captured=true;
            let mut output=io::stdout().lock();
            writeln!(output,"P\t1\t1")?;
            writeln!(output,"D\t{digest}")?;
            output.flush()?;
            control.stop();
            Ok(())
        }
    }

    pub(super) fn run()->Result<(),Box<dyn Error+Send+Sync>>{
        // A normal user cannot invoke an arbitrary capture command through
        // this binary. Its only operational mode is the host-owned reviewed
        // one-shot picker and it requires interactive OS picker selection.
        let args:Vec<_>=env::args_os().skip(1).collect();
        if args.len()!=2||
            !(args[0]=="--lu-observe-one-shot"
                ||args[0]=="--lu-observe-foreground-one-shot"){
            return Err("Invalid Lu capture worker operation".into());
        }
        let nonce=args[1].to_string_lossy();
        if nonce.len()!=32 ||!nonce.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err("Invalid host-generated capture nonce".into());
        }
        if args[0]=="--lu-observe-foreground-one-shot"{
            // User explicitly approved this delay through the Lu-native
            // confirmation dialog and should focus the target window.
            // This is NOT an automatic window enumeration/capture loop.
            std::thread::sleep(Duration::from_secs(4));
            let window=Window::foreground()?;
            if !window.is_valid(){return Err("Foreground window is not capturable".into());}
            let pid=window.process_id()?;
            if pid==0||pid==std::process::id(){
                return Err("Foreground window must be a separate selected app".into());
            }
            let (width,height)=(window.width()?,window.height()?);
            if width<=0||height<=0
                ||width>MAX_FRAME_WIDTH as i32||height>MAX_FRAME_HEIGHT as i32{
                return Err("Selected window exceeds Lu's size limit".into());
            }
            let identity=(window.as_raw_hwnd() as usize,pid);
            let settings=Settings::new(
                window,
                CursorCaptureSettings::WithoutCursor,
                DrawBorderSettings::WithBorder,
                SecondaryWindowSettings::Exclude,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                (nonce.into_owned(),Some(identity)),
            );
            return OneFrame::start(settings)
                .map_err(|e|format!("Foreground WGC capture failed: {e:?}").into());
        }
        let Some(picked)=GraphicsCapturePicker::pick_item()? else{
            return Err("Windows capture picker canceled by user".into());
        };
        let (width,height)=picked.size()?;
        if width<=0||height<=0
            ||width>MAX_FRAME_WIDTH as i32||height>MAX_FRAME_HEIGHT as i32{
            return Err("Selected source exceeds Lu's capture size limit".into());
        }
        let settings=Settings::new(
            picked,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithBorder,
            SecondaryWindowSettings::Exclude,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            (nonce.into_owned(),None),
        );
        OneFrame::start(settings).map_err(|e|format!("WGC capture failed: {e:?}").into())
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error)=windows_capture_worker::run(){
        // Do not echo OS window titles or sensitive WinRT error metadata;
        // only a coarse failure code is visible in Lu's task ledger.
        let _=error;
        eprintln!("Lu WGC selection or bounded frame capture failed");
        std::process::exit(1);
    }
}
