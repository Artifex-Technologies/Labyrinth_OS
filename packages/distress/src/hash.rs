//! Password hashing with the system's libcrypt (yescrypt, the same as /etc/shadow on Arch). No password is stored anywhere in the clear.

use std::ffi::{c_char, c_int, c_ulong, CStr, CString};

#[link(name = "crypt")]
extern "C" {
    fn crypt_gensalt(prefix: *const c_char, count: c_ulong, rbytes: *const c_char, nrbytes: c_int) -> *mut c_char;
    fn crypt(phrase: *const c_char, setting: *const c_char) -> *mut c_char;
}

/// libcrypt hands back a buffer it owns, so only one thread may use it at a time.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn random(n: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut b = vec![0u8; n];
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).map_err(|e| e.to_string())?;
    Ok(b)
}

pub fn hash(password: &str) -> Result<String, String> {
    let pw = CString::new(password).map_err(|_| "the password contains a NUL")?;
    let rnd = random(32)?;
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        let salt = crypt_gensalt(c"$y$".as_ptr(), 0, rnd.as_ptr() as *const c_char, rnd.len() as c_int);
        if salt.is_null() {
            return Err("the system could not make a salt".into());
        }
        let h = crypt(pw.as_ptr(), salt);
        if h.is_null() || *h == b'*' as c_char {
            return Err("the system could not hash the password".into());
        }
        Ok(CStr::from_ptr(h).to_string_lossy().into_owned())
    }
}

/// True when `password` hashes to `stored`. The comparison does not stop at the first difference.
pub fn verify(password: &str, stored: &str) -> bool {
    let (Ok(pw), Ok(st)) = (CString::new(password), CString::new(stored)) else { return false };
    if stored.is_empty() {
        return false;
    }
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        let h = crypt(pw.as_ptr(), st.as_ptr());
        if h.is_null() || *h == b'*' as c_char {
            return false;
        }
        let got = CStr::from_ptr(h).to_bytes();
        let want = stored.as_bytes();
        let mut diff = got.len() ^ want.len();
        for i in 0..got.len().min(want.len()) {
            diff |= (got[i] ^ want[i]) as usize;
        }
        diff == 0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        let h = super::hash("correct horse").unwrap();
        assert!(h.starts_with("$y$"));
        assert!(super::verify("correct horse", &h));
        assert!(!super::verify("correct hors", &h));
        assert!(!super::verify("x", ""));
    }
}
