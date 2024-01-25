
use std::env;
use fileemu_utoc_stream_emulator;
#[allow(unused_imports)]
use windows::Win32::Foundation::HANDLE;
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

    fileemu_utoc_stream_emulator::add_from_folders(&test_mod_1);
    fileemu_utoc_stream_emulator::add_from_folders(&test_mod_2);
    fileemu_utoc_stream_emulator::build_table_of_contents(HANDLE::default(), &unreal_essentials_toc);
}
