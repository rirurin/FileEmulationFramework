# globals
$csharp_folder = "UTOC.Stream.Emulator"
$rust_lib_folder = "fileemu-utoc-stream-emulator"
$mod_folder_name = "reloaded.universal.fileemulationframework.utoc"

# build C# project
dotnet build "./$csharp_folder/$csharp_folder.csproj" -v q -c Debug 
# build Rust projects
cargo +nightly build --target x86_64-pc-windows-msvc # build for both targets in workspace
Push-Location "./$rust_lib_folder" # but additionally build library into R-II mod directory
cargo +nightly build --lib --target x86_64-pc-windows-msvc -Z unstable-options --out-dir "$env:RELOADEDIIMODS\$mod_folder_name\"
Pop-Location
