@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

echo.
echo ========================================
echo     BinWalker 固件分析工具构建脚本
echo ========================================
echo.

if "%1"=="" goto :check
if "%1"=="dev" goto :dev
if "%1"=="build" goto :build
if "%1"=="clean" goto :clean
if "%1"=="run" goto :run
if "%1"=="check" goto :check

echo 用法: build.bat [命令]
echo.
echo 命令:
echo   check  - 检查开发环境 (默认)
echo   dev    - 启动开发模式 (热重载)
echo   build  - 构建生产版本
echo   run    - 运行已构建的程序
echo   clean  - 清理构建缓存
echo.
goto :end

:check
echo 检查开发环境...
echo.

where node >nul 2>&1
if %errorlevel% equ 0 (
    for /f "tokens=*" %%i in ('node --version') do set NODE_VER=%%i
    echo   [OK] Node.js: !NODE_VER!
) else (
    echo   [FAIL] Node.js 未安装
    echo        安装: winget install OpenJS.NodeJS.LTS
)

where npm >nul 2>&1
if %errorlevel% equ 0 (
    for /f "tokens=*" %%i in ('npm --version') do set NPM_VER=%%i
    echo   [OK] npm: !NPM_VER!
) else (
    echo   [FAIL] npm 未安装
)

where rustc >nul 2>&1
if %errorlevel% equ 0 (
    for /f "tokens=*" %%i in ('rustc --version') do set RUST_VER=%%i
    echo   [OK] Rust: !RUST_VER!
) else (
    echo   [FAIL] Rust 未安装
    echo        安装: winget install Rustlang.Rustup
)

where cargo >nul 2>&1
if %errorlevel% equ 0 (
    for /f "tokens=*" %%i in ('cargo --version') do set CARGO_VER=%%i
    echo   [OK] Cargo: !CARGO_VER!
) else (
    echo   [FAIL] Cargo 未安装
)

echo.
echo 环境检查完成!
echo.
goto :end

:dev
echo 启动开发模式...
echo   按 Ctrl+C 停止
echo.

if not exist "node_modules" (
    echo 安装 npm 依赖...
    call npm install
    if errorlevel 1 (
        echo npm 依赖安装失败
        goto :end
    )
)

call npm run tauri dev
goto :end

:build
echo 构建生产版本...
echo.

if not exist "node_modules" (
    echo 安装 npm 依赖...
    call npm install
    if errorlevel 1 (
        echo npm 依赖安装失败
        goto :end
    )
)

echo 正在编译...
call npm run tauri build

if %errorlevel% equ 0 (
    echo.
    echo ========================================
    echo   构建成功!
    echo ========================================
    echo.
    echo 输出位置:
    echo   src-tauri\target\release\BinWalker.exe
    echo.
    echo 安装包位置:
    echo   src-tauri\target\release\bundle\nsis\
    echo.
) else (
    echo 构建失败
)
goto :end

:clean
echo 清理构建缓存...
echo.

if exist "src-tauri\target" (
    rmdir /s /q "src-tauri\target"
    echo   已清理 Rust 构建缓存
)

if exist "dist" (
    rmdir /s /q "dist"
    echo   已清理前端构建缓存
)

if exist "node_modules" (
    set /p CONFIRM="是否删除 node_modules? (y/N): "
    if /i "!CONFIRM!"=="y" (
        rmdir /s /q "node_modules"
        echo   已删除 node_modules
    )
)

echo 清理完成!
goto :end

:run
set EXE_PATH=src-tauri\target\release\BinWalker.exe

if exist "!EXE_PATH!" (
    echo 启动 BinWalker...
    echo.
    start "" "!EXE_PATH!"
) else (
    echo 未找到可执行文件: !EXE_PATH!
    echo 请先运行: build.bat build
)
goto :end

:end
echo.
endlocal
