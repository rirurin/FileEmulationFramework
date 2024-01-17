# globals
$csharp_folder = "UTOC.Stream.Emulator"
$rust_folder = "fileemu-utoc-stream-emulator"
$mod_folder_name = "reloaded.universal.fileemulationframework.utoc"

# build C# project
dotnet build "./$csharp_folder/$csharp_folder.csproj" -v q -c Debug 
# build Rust project
Push-Location "./$rust_folder"
cargo +nightly build --lib --target x86_64-pc-windows-msvc -Z unstable-options --out-dir "$env:RELOADEDIIMODS\$mod_folder_name\"
Pop-Location