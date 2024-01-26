pub struct TArray;

// Convert a TArray in a byte stream into a Vec
pub trait TArrayDeserializer {

}
// Convert a Vec into a serialized TArray. Length is always stored in bytes
pub trait TArraySerializer {
    fn to_buffer_entries<W: Write, T, E: byteorder::ByteOrder>(rstr: &Vec<T>, writer: &mut W) -> Result<(), Box<dyn Error>>;
    //fn to_buffer_bytes<W: Write, T, E: byteorder::ByteOrder>(rstr: &Vec<T>, writer: &mut W) -> Result<(), Box<dyn Error>>;
}