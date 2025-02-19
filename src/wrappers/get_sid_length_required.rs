/// Wraps [`GetSidLengthRequired`](https://docs.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getsidlengthrequired)
#[allow(non_snake_case)]
pub fn GetSidLengthRequired(sub_auth_count: u8) -> usize {
    // Assumptions:
    // - None. The function is guaranteed by the WinAPI not to fail
    unsafe { winapi::um::securitybaseapi::GetSidLengthRequired(sub_auth_count) as usize }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn example_sid_lens() {
        for count in 0..std::u8::MAX {
            assert_eq!(GetSidLengthRequired(count), 8 + 4 * count as usize);
        }
    }
}
