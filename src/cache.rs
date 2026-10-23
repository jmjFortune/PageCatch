//! Drop the kernel's page cache for a finished video.
//!
//! Strict verification decodes the whole output once, and publishing copies it
//! again, so the container's cgroup can keep hundreds of megabytes of clean file
//! pages after a download. Those pages are reclaimable cache rather than leaked
//! heap, but on a NAS it is useful to hand them back as soon as the task ends.

use std::path::Path;

/// Ask the kernel to forget cached pages for `path` without changing the file.
/// A no-op outside Linux; dirty pages are written back by the kernel first.
pub fn release(path: &Path) {
    #[cfg(target_os = "linux")]
    if let Ok(file) = std::fs::File::open(path) {
        use std::os::unix::io::AsRawFd;
        // SAFETY: the descriptor is open for the duration of the call and the
        // advice only affects cached pages, never the file contents.
        unsafe {
            libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::release;

    #[test]
    fn releasing_cache_keeps_the_file_intact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("finished.mp4");
        std::fs::write(&path, b"media bytes").unwrap();
        release(&path);
        assert_eq!(std::fs::read(&path).unwrap(), b"media bytes");
    }
}
