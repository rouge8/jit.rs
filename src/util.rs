pub fn is_executable(mode: u32) -> bool {
    mode & 0o1111 != 0
}
