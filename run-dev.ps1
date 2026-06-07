$winlibsBin = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$llvmBin = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\llvm-mingw-20260602-ucrt-x86_64\bin"

$env:PATH = "$winlibsBin;$llvmBin;$env:PATH"

Write-Host "=== 环境已设置 ===" -ForegroundColor Green
Write-Host "MinGW 工具链: $winlibsBin" -ForegroundColor Cyan
Write-Host "LLVM 工具链  : $llvmBin" -ForegroundColor Cyan

Write-Host "`n启动 Tauri 开发模式..." -ForegroundColor Yellow
npm run tauri dev
