use crate::kprintln;
use crate::mm::PageFlags;
use crate::sched::task::AddressSpace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserPtrError {
    BadAddress,
    NotMapped,
    NotWritable,
    OverflowDetected,
}

impl From<UserPtrError> for i64 {
    fn from(err: UserPtrError) -> i64 {
        match err {
            UserPtrError::BadAddress => -14,
            UserPtrError::NotMapped => -14,
            UserPtrError::NotWritable => -13,
            UserPtrError::OverflowDetected => -22,
        }
    }
}

const USER_MAX: u64 = 0x0000_8000_0000_0000;

pub fn validate_user_slice<'a>(
    aspace: &AddressSpace,
    ptr: u64,
    len: u64,
) -> Result<&'a [u8], UserPtrError> {
    if ptr == 0 {
        return Err(UserPtrError::BadAddress);
    }
    if len == 0 {
        return Ok(&[]);
    }

    let end = ptr.checked_add(len).ok_or(UserPtrError::BadAddress)?;
    if end > USER_MAX {
        kprintln!(
            "[SECURITY] userspace read violation: ptr={:#X} len={} end={:#X}",
            ptr,
            len,
            end
        );
        return Err(UserPtrError::BadAddress);
    }

    let first_page = ptr & !0xFFF;
    let last_page = (end - 1) & !0xFFF;
    let mut page = first_page;
    while page <= last_page {
        aspace.translate(page).ok_or(UserPtrError::NotMapped)?;
        page += 0x1000;
    }

    Ok(unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) })
}

pub fn validate_user_slice_mut<'a>(
    aspace: &AddressSpace,
    ptr: u64,
    len: u64,
) -> Result<&'a mut [u8], UserPtrError> {
    if ptr == 0 {
        return Err(UserPtrError::BadAddress);
    }
    if len == 0 {
        return Ok(&mut []);
    }

    let end = ptr.checked_add(len).ok_or(UserPtrError::BadAddress)?;
    if end > USER_MAX {
        kprintln!(
            "[SECURITY] userspace write violation: ptr={:#X} len={} end={:#X}",
            ptr,
            len,
            end
        );
        return Err(UserPtrError::BadAddress);
    }

    let first_page = ptr & !0xFFF;
    let last_page = (end - 1) & !0xFFF;
    let mut page = first_page;
    while page <= last_page {
        let flags = aspace
            .translate_flags(page)
            .ok_or(UserPtrError::NotMapped)?;
        if !flags.contains(PageFlags::WRITABLE) {
            return Err(UserPtrError::NotWritable);
        }
        page += 0x1000;
    }

    Ok(unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, len as usize) })
}
