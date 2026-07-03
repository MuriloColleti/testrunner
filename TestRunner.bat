@echo off
cd /d "%~dp0"

if exist "src-tauri\target\release\testrunner.exe" (
    start "" "src-tauri\target\release\testrunner.exe"
) else (
    where npm >nul 2>&1 || (echo ERRO: Node/npm nao encontrado. Instale em https://nodejs.org && pause & exit /b 1)
    where cargo >nul 2>&1 || (echo ERRO: Rust/Cargo nao encontrado. Instale em https://rustup.rs && pause & exit /b 1)
    if not exist node_modules call npm install
    npm run dev
)
