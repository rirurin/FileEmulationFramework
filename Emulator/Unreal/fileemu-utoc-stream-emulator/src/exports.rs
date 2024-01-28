use crate::{asset_collector, toc_factory, toc_factory::PartitionBlock};
use std::{
    ffi::CStr,
    os::raw::c_char
};

#[no_mangle]
#[allow(non_snake_case)]
// modId is used by the asset collector profiler
pub unsafe extern "C" fn AddFromFolders(modId: *const c_char, modPath: *const c_char) {
    asset_collector::add_from_folders(CStr::from_ptr(modId).to_str().unwrap(), CStr::from_ptr(modPath).to_str().unwrap());
}

#[no_mangle]
#[allow(non_snake_case)]
// haiiii Reloaded!!!! :3
pub unsafe extern "C" fn BuildTableOfContents(tocPath: *const c_char, settings: *const u32, settings_length: u32, length: *mut u64) -> *const u8 {
    match toc_factory::build_table_of_contents(CStr::from_ptr(tocPath).to_str().unwrap()) {
        Some(n) => {
            unsafe {
                toc_factory::TOC_STREAM = n; // move vector pointer into TOC_STREAM to stop it from dropping
                *length = toc_factory::TOC_STREAM.len() as u64; // set length parameter
            }
            toc_factory::TOC_STREAM.as_ptr()
        },
        None => 0 as *const u8 // send a null pointer :naosmiley:
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn GetVirtualPartition(filePath: *const c_char, block_count: *mut u64, status: *mut u32) -> *const PartitionBlock {
    let blocks = toc_factory::get_virtual_partition(CStr::from_ptr(filePath).to_str().unwrap());
    *block_count = blocks.len() as u64;
    blocks.as_ptr()
}

// Keep in sync with Signatures.cs from UnrealEssentials
// https://github.com/AnimatedSwine37/UnrealEssentials/blob/master/UnrealEssentials/Signatures.cs
// Get Swine to implement an interface method to
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "C" fn SetVersion(branch_name: *const c_char) {
    let valid_unreal_branch_name_maybe = CStr::from_ptr(branch_name).to_str().unwrap();
    match valid_unreal_branch_name_maybe {
        "++UE4+Release-4.25" => (), // 4.25
        "++UE4+Release-4.25Plus M3" => (), // Scarlet Nexus (4.25+)
        "++UE4+Release-4.26" => (), // 4.26
        "++UE4+Release-4.27" => (), // 4.27
        "++ue4+hibiki_patch+4.27hbk" => (), // Hi-Fi RUSH (Update 5)
        "++ue4+ue4_main+4.27hbk" => (), // Hi-Fi RUSH (Update 7)
        _ => ()
    }
}