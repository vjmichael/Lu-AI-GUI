# Reviewed Lu IDE CMake build+CTest driver, packaged INSIDE the sealed runtime.
# This is not evaluated from user project files or a caller-specified path.
# CMake configure may execute arbitrary project code; OS isolation is required.
cmake_minimum_required(VERSION 3.25)
if(NOT DEFINED LU_IDE_WORKSPACE OR NOT DEFINED LU_IDE_OUTPUT)
  message(FATAL_ERROR "DEV03_NO_FIXED_WORKSPACE_OR_OUTPUT")
endif()
get_filename_component(_bindir "${CMAKE_COMMAND}" DIRECTORY)
if(WIN32)
  set(_ninja "${_bindir}/ninja.exe")
  set(_clang "${_bindir}/clang.exe")
  set(_clangxx "${_bindir}/clang++.exe")
  set(_ctest "${_bindir}/ctest.exe")
else()
  set(_ninja "${_bindir}/ninja")
  set(_clang "${_bindir}/clang")
  set(_clangxx "${_bindir}/clang++")
  set(_ctest "${_bindir}/ctest")
endif()
foreach(_tool IN ITEMS "${_ninja}" "${_clang}" "${_clangxx}" "${_ctest}")
  if(NOT EXISTS "${_tool}")
    message(FATAL_ERROR "DEV03_PINNED_TOOL_MISSING")
  endif()
endforeach()
set(_build "${LU_IDE_OUTPUT}/build")
execute_process(COMMAND "${CMAKE_COMMAND}" -S "${LU_IDE_WORKSPACE}"
  -B "${_build}" -G Ninja
  "-DCMAKE_MAKE_PROGRAM=${_ninja}"
  "-DCMAKE_C_COMPILER=${_clang}"
  "-DCMAKE_CXX_COMPILER=${_clangxx}"
  -DCMAKE_BUILD_TYPE=Debug
  RESULT_VARIABLE _configured)
if(NOT "${_configured}" STREQUAL "0")
  message(FATAL_ERROR "DEV03_CMAKE_CONFIGURE_FAILED")
endif()
execute_process(COMMAND "${CMAKE_COMMAND}" --build "${_build}" --parallel 1
  RESULT_VARIABLE _compiled)
if(NOT "${_compiled}" STREQUAL "0")
  message(FATAL_ERROR "DEV03_CMAKE_BUILD_FAILED")
endif()
execute_process(COMMAND "${_ctest}" --test-dir "${_build}" --output-on-failure
  RESULT_VARIABLE _tested)
if(NOT "${_tested}" STREQUAL "0")
  message(FATAL_ERROR "DEV03_CTEST_FAILED")
endif()
message(STATUS "DEV03_CMAKE_CTEST_COMPLETED_INSIDE_WORKER")
