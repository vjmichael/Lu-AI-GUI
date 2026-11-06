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

#[cfg(not(windows))]
fn main() {
    eprintln!("Lu Graphics Capture worker requires interactive Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
mod windows_capture_worker {
    use sha2::{Digest, Sha256};
    use std::{env,error::Error,io::{self,Write},time::Instant};
    use windows_capture::{
        capture::{Context,GraphicsCaptureApiHandler},
        frame::Frame,
        graphics_capture_api::InternalCaptureControl,
        graphics_capture_picker::GraphicsCapturePicker,
        settings::{
            ColorFormat,CursorCaptureSettings,DirtyRegionSettings,
            DrawBorderSettings,MinimumUpdateIntervalSettings,
            SecondaryWindowSettings,Settings
        },
    };

    const MAX_WIDTH:i32=7680;
    const MAX_HEIGHT:i32=4320;
    const MAX_FRAME_BYTES:usize=7680*4320*4;
    const MAX_CAPTURE_SECS:u64=20;

    struct OneFrame {
        started:Instant,
        captured:bool,
        nonce:String,
    }
    impl GraphicsCaptureApiHandler for OneFrame {
        type Flags=String;
        type Error=Box<dyn Error+Send+Sync>;
        fn new(ctx:Context<Self::Flags>)->Result<Self,Self::Error>{
            Ok(Self{started:Instant::now(),captured:false,nonce:ctx.flags})
        }
        fn on_frame_arrived(
            &mut self,frame:&mut Frame,
            control:InternalCaptureControl
        )->Result<(),Self::Error>{
            if self.captured || self.started.elapsed().as_secs()>MAX_CAPTURE_SECS {
                control.stop();
                return Ok(());
            }
            let width=frame.width();
            let height=frame.height();
            let bytes=usize::try_from(width)
                .ok().and_then(|w|usize::try_from(height).ok()
                    .and_then(|h|w.checked_mul(h)?.checked_mul(4)))
                .ok_or("Invalid capture dimensions")?;
            if width==0||height==0||
                width>MAX_WIDTH as u32||height>MAX_HEIGHT as u32||
                bytes>MAX_FRAME_BYTES{
                control.stop();
                return Err("Capture dimensions exceed Lu's bounded frame limit".into());
            }
            let mut gpu_frame=frame.buffer()?;
            let buffer=gpu_frame.as_raw_buffer();
            if buffer.len()<bytes||buffer.len()>MAX_FRAME_BYTES+(MAX_HEIGHT as usize*1024){
                control.stop();
                return Err("Invalid capture row pitch or frame size".into());
            }
            // Read actual WGC/D3D11-mapped bytes. No screenshot file, video
            // recording, user text, clipboard, window title or pixel IPC.
            // This hash is a *capture receipt*, not visual understanding.
            let mut sha=Sha256::new();
            sha.update(b"Lu-WGC-one-shot-frame-v1");
            sha.update(self.nonce.as_bytes());
            sha.update(width.to_le_bytes());
            sha.update(height.to_le_bytes());
            sha.update(buffer);
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
        if args.len()!=2||args[0]!="--lu-observe-one-shot"{
            return Err("Invalid Lu capture worker operation".into());
        }
        let nonce=args[1].to_string_lossy();
        if nonce.len()!=32 ||!nonce.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err("Invalid host-generated capture nonce".into());
        }
        let Some(picked)=GraphicsCapturePicker::pick_item()? else{
            return Err("Windows capture picker canceled by user".into());
        };
        let (width,height)=picked.size()?;
        if width<=0||height<=0||width>MAX_WIDTH||height>MAX_HEIGHT{
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
            nonce.into_owned(),
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
