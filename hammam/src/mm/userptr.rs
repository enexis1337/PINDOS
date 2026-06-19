/// Валидация и безопасная работа с userspace указателями
/// 
/// Userspace адреса в x86-64: 0x0000_0000_0000_0000 - 0x0000_7FFF_FFFF_FFFF
/// (canonical user range, ниже 0x0000_8000_0000_0000)

use crate::kprintln;

/// Ошибки при работе с userspace памятью
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserPtrError {
    BadAddress,        // Адрес вне userspace диапазона
    NotMapped,         // Страница не отображена в address space
    NotWritable,       // Страница не имеет флага WRITABLE
    OverflowDetected,  // Обнаружено целочисленное переполнение
}

impl From<UserPtrError> for i64 {
    fn from(err: UserPtrError) -> i64 {
        match err {
            UserPtrError::BadAddress => -14,       // EFAULT
            UserPtrError::NotMapped => -14,        // EFAULT
            UserPtrError::NotWritable => -13,      // EACCES
            UserPtrError::OverflowDetected => -22, // EINVAL
        }
    }
}

/// Максимальный адрес userspace (0x0000_8000_0000_0000)
const USER_MAX: u64 = 0x0000_8000_0000_0000;

/// Размер страницы
const PAGE_SIZE: u64 = 4096;

/// Проверить что диапазон [ptr, ptr+len) полностью в userspace
/// и что все страницы в диапазоне отображены.
pub fn validate_user_slice(ptr: u64, len: u64) -> Result<&'static [u8], UserPtrError> {
    // Проверка 1: нулевой указатель не допускается
    if ptr == 0 {
        return Err(UserPtrError::BadAddress);
    }

    // Проверка 2: пустой slice разрешен
    if len == 0 {
        return Ok(&[]);
    }

    // Проверка 3: проверить переполнение при вычислении конца
    let end = ptr.checked_add(len).ok_or(UserPtrError::OverflowDetected)?;

    // Проверка 4: весь диапазон должен быть в userspace
    if end > USER_MAX {
        kprintln!(
            "[SECURITY] userspace read violation: ptr={:#X} len={} end={:#X}",
            ptr,
            len,
            end
        );
        return Err(UserPtrError::BadAddress);
    }

    // Проверка 5: проверить что все страницы отображены
    // Итерируем по страницам: от (ptr / PAGE_SIZE) до ((end - 1) / PAGE_SIZE)
    let start_page = ptr / PAGE_SIZE;
    let end_page = (end - 1) / PAGE_SIZE;

    for page_idx in start_page..=end_page {
        let page_vaddr = page_idx * PAGE_SIZE;
        
        // Используем translate из mm::paging для проверки что страница отображена
        if crate::mm::translate(page_vaddr).is_none() {
            kprintln!(
                "[SECURITY] userspace page not mapped: vaddr={:#X}",
                page_vaddr
            );
            return Err(UserPtrError::NotMapped);
        }
    }

    // SAFETY: проверили что:
    // 1. диапазон в userspace
    // 2. все страницы отображены в address space текущего процесса
    // 3. нет переполнения
    Ok(unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) })
}

/// Проверить что диапазон [ptr, ptr+len) полностью в userspace
/// и что все страницы в диапазоне отображены И WRITABLE.
pub fn validate_user_slice_mut(
    ptr: u64,
    len: u64,
) -> Result<&'static mut [u8], UserPtrError> {
    // Проверка 1: нулевой указатель не допускается
    if ptr == 0 {
        return Err(UserPtrError::BadAddress);
    }

    // Проверка 2: пустой slice разрешен
    if len == 0 {
        return Ok(&mut []);
    }

    // Проверка 3: проверить переполнение при вычислении конца
    let end = ptr.checked_add(len).ok_or(UserPtrError::OverflowDetected)?;

    // Проверка 4: весь диапазон должен быть в userspace
    if end > USER_MAX {
        kprintln!(
            "[SECURITY] userspace write violation: ptr={:#X} len={} end={:#X}",
            ptr,
            len,
            end
        );
        return Err(UserPtrError::BadAddress);
    }

    // Проверка 5: проверить что все страницы отображены И writable
    let start_page = ptr / PAGE_SIZE;
    let end_page = (end - 1) / PAGE_SIZE;

    for page_idx in start_page..=end_page {
        let page_vaddr = page_idx * PAGE_SIZE;

        // Проверить наличие страницы
        if crate::mm::translate(page_vaddr).is_none() {
            kprintln!(
                "[SECURITY] userspace page not mapped for write: vaddr={:#X}",
                page_vaddr
            );
            return Err(UserPtrError::NotMapped);
        }

        // TODO: проверить WRITABLE флаг через translate + page table lookup
        // Сейчас у нас нет полного API для проверки флагов, только адреса перевода
        // В будущем нужно добавить mm::translate_with_flags(vaddr) -> (phys_addr, flags)
    }

    // SAFETY: проверили что:
    // 1. диапазон в userspace
    // 2. все страницы отображены в address space текущего процесса
    // 3. нет переполнения
    Ok(unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, len as usize) })
}

/// Копировать данные из userspace памяти в kernel памяти (безопасно)
pub fn copy_from_user(user_ptr: u64, len: usize) -> Result<alloc::vec::Vec<u8>, UserPtrError> {
    let user_slice = validate_user_slice(user_ptr, len as u64)?;
    Ok(alloc::vec::Vec::from(user_slice))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_pointer_rejected() {
        assert_eq!(
            validate_user_slice(0, 100),
            Err(UserPtrError::BadAddress)
        );
    }

    #[test]
    fn test_empty_slice_allowed() {
        let result = validate_user_slice(0x1000, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_overflow_detection() {
        // ptr + len переполняется
        let result = validate_user_slice(0x0000_FFFF_FFFF_0000, 0x2000);
        assert_eq!(result, Err(UserPtrError::OverflowDetected));
    }

    #[test]
    fn test_out_of_userspace_range() {
        // Адрес вне userspace диапазона
        let kernel_addr = 0xFFFF_8000_0000_0000u64;
        let result = validate_user_slice(kernel_addr, 100);
        assert_eq!(result, Err(UserPtrError::BadAddress));
    }

    #[test]
    fn test_range_crossing_boundary() {
        // Диапазон начинается в userspace но заканчивается в kernel space
        let ptr = USER_MAX - 50;
        let len = 100;
        let result = validate_user_slice(ptr, len);
        assert_eq!(result, Err(UserPtrError::BadAddress));
    }
}
