use crate::constants::{SeObjectType, SecurityInformation};
use crate::{wrappers, Acl, LocalBox, Sid};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::io;
use std::ptr::NonNull;
use std::str::FromStr;
use winapi::ctypes::c_void;
use winapi::um::winnt::SECURITY_DESCRIPTOR;

pub struct SecurityDescriptor {
    _inner: SECURITY_DESCRIPTOR,
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unreachable!("SecurityDescriptor should only be borrowed, not owned")
    }
}

impl SecurityDescriptor {
    /// Get the security descriptor for a file at a given path
    ///
    /// This is a direct call to `wrappers::GetNamedSecurityInfo` with some
    /// default parameters. For more options (such as fetching a partial
    /// descriptor, or getting descriptors for other objects), call that method
    /// directly.
    pub fn lookup_path<S: AsRef<OsStr> + ?Sized>(
        path: &S,
    ) -> io::Result<LocalBox<SecurityDescriptor>> {
        wrappers::GetNamedSecurityInfo(
            path.as_ref(),
            SeObjectType::SE_FILE_OBJECT,
            SecurityInformation::Dacl | SecurityInformation::Owner | SecurityInformation::Group,
        )
    }

    /// Get the security descriptor for a file
    ///
    /// This is a direct call to `wrappers::GetSecurityInfo` with some default
    /// parameters. For more options, call that method directly.
    pub fn lookup_file(file: &File) -> io::Result<LocalBox<SecurityDescriptor>> {
        wrappers::GetSecurityInfo(
            file,
            SeObjectType::SE_FILE_OBJECT,
            SecurityInformation::Dacl | SecurityInformation::Owner | SecurityInformation::Group,
        )
    }
}

impl SecurityDescriptor {
    pub unsafe fn ref_from_nonnull<'s>(ptr: NonNull<c_void>) -> &'s Self {
        let sd_ref = std::mem::transmute::<NonNull<c_void>, &Self>(ptr);
        debug_assert!(wrappers::IsValidSecurityDescriptor(sd_ref));
        sd_ref
    }

    /// Get the Security Descriptor Definition Language (SDDL) string
    /// corresponding to this `SecurityDescriptor`
    ///
    /// This function attempts to get the entire SDDL string using
    /// `SecurityInformation::all()`. To get a portion of the SDDL, use
    /// `wrappers::ConvertSecurityDescriptorToStringSecurityDescriptor`
    /// directly.
    pub fn as_sddl(&self) -> io::Result<OsString> {
        wrappers::ConvertSecurityDescriptorToStringSecurityDescriptor(
            self,
            SecurityInformation::all(),
        )
    }

    /// Get the owner SID if it exists
    pub fn owner(&self) -> Option<&Sid> {
        wrappers::GetSecurityDescriptorOwner(self)
            .expect("Valid SecurityDescriptor failed to get owner")
    }

    /// Get the group SID if it exists
    pub fn group(&self) -> Option<&Sid> {
        wrappers::GetSecurityDescriptorGroup(self)
            .expect("Valid SecurityDescriptor failed to get group")
    }

    /// Get the DACL if it exists
    pub fn dacl(&self) -> Option<&Acl> {
        wrappers::GetSecurityDescriptorDacl(self)
            .expect("Valid SecurityDescriptor failed to get dacl")
    }

    /// Get the SACL if it exists
    pub fn sacl(&self) -> Option<&Acl> {
        wrappers::GetSecurityDescriptorSacl(self)
            .expect("Valid SecurityDescriptor failed to get sacl")
    }
}

impl fmt::Debug for SecurityDescriptor {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_map()
            .entry(&"owner", &self.owner())
            .entry(&"group", &self.group())
            .entry(&"sddl", &self.as_sddl().unwrap())
            .finish()
    }
}

impl FromStr for LocalBox<SecurityDescriptor> {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        wrappers::ConvertStringSecurityDescriptorToSecurityDescriptor(s)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::LocalBox;
    use std::ops::Deref;

    static SDDL_TEST_CASES: &[(&str, &str, &str)] = &[
        ("", "", ""),
        ("O:AOG:SY", "AO", "SY"),
        ("O:SU", "SU", ""),
        ("G:SI", "", "SI"),
        ("O:AOG:SYD:S:", "AO", "SY"),
    ];

    fn assert_option_eq(lhs: Option<&Sid>, rhs: Option<&LocalBox<Sid>>) {
        match (lhs, rhs) {
            (None, None) => (),
            (Some(_), None) => panic!("Assertion failed: {:?} == {:?}", lhs, rhs),
            (None, Some(_)) => panic!("Assertion failed: {:?} == {:?}", lhs, rhs),
            (Some(l), Some(r)) => assert_eq!(l, r.deref()),
        }
    }

    fn sddl_test_cases(
    ) -> impl Iterator<Item = (String, Option<LocalBox<Sid>>, Option<LocalBox<Sid>>)> {
        let parse_if_there = |s: &str| {
            if s.is_empty() {
                None
            } else {
                Some(s.parse().unwrap())
            }
        };

        SDDL_TEST_CASES.iter().map(move |(sddl, own, grp)| {
            (sddl.to_string(), parse_if_there(own), parse_if_there(grp))
        })
    }

    #[test]
    fn sddl_get_sids() -> io::Result<()> {
        for (sddl, owner, group) in sddl_test_cases() {
            let sd: LocalBox<SecurityDescriptor> = sddl.parse()?;

            assert_option_eq(sd.owner(), owner.as_ref());
            assert_option_eq(sd.group(), group.as_ref());
        }

        Ok(())
    }

    #[test]
    fn sddl_round_trip() -> io::Result<()> {
        for (sddl, _, _) in sddl_test_cases() {
            let sd: LocalBox<SecurityDescriptor> = sddl.parse()?;
            let sddl2 = sd.as_sddl()?;

            assert_eq!(OsStr::new(&sddl), &sddl2);
        }

        Ok(())
    }

    #[test]
    fn sddl_missing_acls() -> io::Result<()> {
        let sd: LocalBox<SecurityDescriptor> = "O:LAG:AO".parse()?;
        assert!(sd.dacl().is_none());
        assert!(sd.sacl().is_none());

        let sd: LocalBox<SecurityDescriptor> = "O:LAG:AOD:".parse()?;
        assert!(sd.dacl().is_some());
        assert!(sd.sacl().is_none());

        let sd: LocalBox<SecurityDescriptor> = "O:LAG:AOS:".parse()?;
        assert!(sd.dacl().is_none());
        assert!(sd.sacl().is_some());

        let sd: LocalBox<SecurityDescriptor> = "O:LAG:AOD:S:".parse()?;
        assert!(sd.dacl().is_some());
        assert!(sd.sacl().is_some());

        Ok(())
    }
}
