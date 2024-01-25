
use std::{
    env,
    time::Instant
};
use fileemu_utoc_stream_emulator;
#[allow(unused_imports, unused_braces)]
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::HANDLE,
        Storage::FileSystem
    }
};
#[allow(unused_variables)]
fn main() {
    // TOC building test program without having to start Reloaded
    let r3_mod_path = env::var("RELOADEDIIMODS");
    if let Err(_) = r3_mod_path {
        println!("Environment variables is missing an entry for \"RELOADEDIIMODS\"");
        return;
    }
    let r3_mod_path = r3_mod_path.unwrap();
    println!("Using Reloaded mods directory at {}", r3_mod_path);
    // get our first test mods, convert to byte*
    let test_mod_1 = r3_mod_path.clone() + "/uetest.loosefiletest1";
    let test_mod_2 = r3_mod_path.clone() + "/uetest.loosefiletest2";
    let unreal_essentials_toc = r3_mod_path.clone() + "/UnrealEssentials/Unreal/UnrealEssentials_P.utoc";
    let unreal_essentials_partition = r3_mod_path.clone() + "/UnrealEssentials/Unreal/UnrealEssentials_P.ucas";

    // open TOC file handle
    let toc_filename_win32 = unreal_essentials_toc.clone() + "\0";
    let start_making_toc = Instant::now();
    unsafe {
        // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
        // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/FileSystem/fn.CreateFileA.html
        match FileSystem::CreateFileA(
            PCSTR::from_raw(toc_filename_win32.as_ptr()),
            FileSystem::FILE_GENERIC_WRITE.0,
            FileSystem::FILE_SHARE_MODE(0),
            None,
            FileSystem::FILE_CREATION_DISPOSITION(FileSystem::CREATE_ALWAYS.0),
            FileSystem::FILE_FLAGS_AND_ATTRIBUTES(FileSystem::FILE_ATTRIBUTE_NORMAL.0),
            HANDLE::default()
        ) {
            Ok(handle) => {
                println!("Got TOC handle!");
                fileemu_utoc_stream_emulator::add_from_folders(&test_mod_1);
                fileemu_utoc_stream_emulator::add_from_folders(&test_mod_2);
                fileemu_utoc_stream_emulator::build_table_of_contents(handle, &unreal_essentials_toc, &unreal_essentials_partition);
            }
            Err(e) => println!("Error occurred trying to open file: {}", e.to_string())
        }
    }
    // Damn it's slow (25 ms to merge 6 files) this is not the Reloaded3 grindset
    println!("Created Unreal Essentials TOC in {} ms", start_making_toc.elapsed().as_millis());
}
