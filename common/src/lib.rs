use jj_lib::content_hash::ContentHash;

pub fn blake3_hash(x: &(impl ContentHash + ?Sized)) -> digest::Output<blake3::Hasher> {
    use digest::Digest;
    let mut hasher = blake3::Hasher::new();
    x.hash(&mut hasher);
    hasher.finalize()
}
