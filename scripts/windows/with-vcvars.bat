@echo off
rem Wraps a command in the VS 2022 Build Tools x64 environment.
rem Required because rustc auto-detects VS 2026 Community's cl.exe but that
rem install lacks the Windows SDK, so linker can't find msvcrt.lib / stdarg.h.
rem See apps/linows/BUILDING.md (Windows section).
rem
rem Usage:   scripts\windows\with-vcvars.bat <command> [args...]
rem Override path: set VCVARSALL=...\vcvarsall.bat
setlocal
if not defined VCVARSALL set "VCVARSALL=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
if not exist "%VCVARSALL%" (
    echo with-vcvars: VCVARSALL not found at "%VCVARSALL%" 1>&2
    echo Set VCVARSALL=^<path^>\vcvarsall.bat or install VS 2022 Build Tools. 1>&2
    exit /b 1
)
call "%VCVARSALL%" x64 >nul 2>&1
if errorlevel 1 (
    echo with-vcvars: vcvarsall.bat x64 failed 1>&2
    exit /b 1
)
%*
exit /b %ERRORLEVEL%
