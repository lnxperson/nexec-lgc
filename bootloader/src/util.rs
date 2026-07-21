pub fn normalize_path(path: &str) -> alloc::string::String {
    path.replace('/', "\\")
}
