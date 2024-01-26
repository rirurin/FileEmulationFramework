use byteorder::WriteBytesExt;
use std::{
    error::Error,
    io::{Seek, Write}
};

pub trait ObjectSerialize {
    fn to_buffer<W: Write + Seek, T, E: byteorder::ByteOrder>(rstr: &Vec<T>, writer: &mut W) -> Result<(), Box<dyn Error>>;
}

pub struct Test;
impl ObjectSerialize for Test {
    fn to_buffer<W: Write + Seek, T, E: byteorder::ByteOrder>(rstr: &Vec<T>, writer: &mut W) -> Result<(), Box<dyn Error>> {
        let a = writer.write_u32::<E>(0);
        Ok(())
    }
}