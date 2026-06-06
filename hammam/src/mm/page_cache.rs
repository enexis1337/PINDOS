use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use spin::Mutex;

/// Идентификатор inode (файла)
type InodeId = u64;

/// Одна кэшированная страница (4 KiB)
#[derive(Clone)]
pub struct CachedPage {
    pub data: [u8; 4096],  // Данные страницы
    pub dirty: bool,       // Флаг грязной страницы (требует записи на диск)
}

impl CachedPage {
    /// Создать новую чистую страницу
    pub fn new(data: [u8; 4096]) -> Self {
        CachedPage { data, dirty: false }
    }

    /// Пометить страницу как грязную
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Пометить страницу как чистую
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
}

/// Единый глобальный кэш страниц (page cache = block cache = vnode cache).
/// Ключ: (inode_id, page_index) — уникально идентифицирует страницу файла.
/// Используется для кэширования данных из любой файловой системы.
pub struct PageCache {
    pages: BTreeMap<(InodeId, u64), Arc<Mutex<CachedPage>>>,
}

impl PageCache {
    /// Создать новый пустой page cache
    pub const fn new() -> Self {
        Self {
            pages: BTreeMap::new(),
        }
    }

    /// Получить страницу из кэша или загрузить через loader-замыкание.
    /// Если страница уже в кэше, возвращаем её.
    /// Если нет, вызываем loader() для загрузки данных и кэшируем результат.
    pub fn get_or_load<F>(
        &mut self,
        inode: InodeId,
        page_idx: u64,
        loader: F,
    ) -> Arc<Mutex<CachedPage>>
    where
        F: FnOnce() -> [u8; 4096],
    {
        self.pages
            .entry((inode, page_idx))
            .or_insert_with(|| {
                Arc::new(Mutex::new(CachedPage {
                    data: loader(),
                    dirty: false,
                }))
            })
            .clone()
    }

    /// Получить страницу из кэша если она есть
    pub fn get(&self, inode: InodeId, page_idx: u64) -> Option<Arc<Mutex<CachedPage>>> {
        self.pages.get(&(inode, page_idx)).cloned()
    }

    /// Вставить страницу в кэш (перезаписывает если была)
    pub fn insert(
        &mut self,
        inode: InodeId,
        page_idx: u64,
        page: CachedPage,
    ) -> Option<Arc<Mutex<CachedPage>>> {
        self.pages
            .insert((inode, page_idx), Arc::new(Mutex::new(page)))
    }

    /// Вытеснить (удалить) страницу из кэша
    pub fn evict(&mut self, inode: InodeId, page_idx: u64) -> bool {
        self.pages.remove(&(inode, page_idx)).is_some()
    }

    /// Вытеснить все страницы inode'а
    pub fn evict_inode(&mut self, inode: InodeId) -> usize {
        let initial_count = self.pages.len();
        
        // Собираем ключи для удаления
        let keys_to_remove: Vec<_> = self
            .pages
            .keys()
            .filter(|(id, _)| *id == inode)
            .cloned()
            .collect();

        // Удаляем найденные ключи
        for key in keys_to_remove {
            self.pages.remove(&key);
        }

        initial_count - self.pages.len()
    }

    /// Получить список всех грязных страниц
    pub fn get_dirty_pages(&self) -> Vec<(InodeId, u64)> {
        self.pages
            .iter()
            .filter_map(|((inode, idx), page_mutex)| {
                let page = page_mutex.lock();
                if page.dirty {
                    Some((*inode, *idx))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Пометить страницу как грязную
    pub fn mark_dirty(&self, inode: InodeId, page_idx: u64) -> bool {
        if let Some(page_arc) = self.pages.get(&(inode, page_idx)) {
            let mut page = page_arc.lock();
            page.mark_dirty();
            true
        } else {
            false
        }
    }

    /// Пометить страницу как чистую
    pub fn mark_clean(&self, inode: InodeId, page_idx: u64) -> bool {
        if let Some(page_arc) = self.pages.get(&(inode, page_idx)) {
            let mut page = page_arc.lock();
            page.mark_clean();
            true
        } else {
            false
        }
    }

    /// Синхронизировать (записать на диск) грязные страницы inode'а
    /// Это заполнитель — реальная реализация требует I/O подсистемы
    pub fn sync_inode(&mut self, inode: InodeId) -> usize {
        let mut synced = 0;
        
        let dirty_keys: Vec<_> = self
            .pages
            .iter()
            .filter_map(|((id, idx), page_mutex)| {
                let page = page_mutex.lock();
                if *id == inode && page.dirty {
                    Some((*id, *idx))
                } else {
                    None
                }
            })
            .collect();

        for (inode, page_idx) in dirty_keys {
            // TODO: реальная запись на диск через I/O подсистему
            if let Some(page_arc) = self.pages.get(&(inode, page_idx)) {
                let mut page = page_arc.lock();
                page.mark_clean();
                synced += 1;
            }
        }

        synced
    }

    /// Получить количество кэшированных страниц
    pub fn cached_pages(&self) -> usize {
        self.pages.len()
    }

    /// Получить количество грязных страниц
    pub fn dirty_pages_count(&self) -> usize {
        self.pages
            .values()
            .filter(|page_arc| page_arc.lock().dirty)
            .count()
    }

    /// Очистить весь кэш (опасная операция!)
    pub fn clear(&mut self) {
        self.pages.clear();
    }

    /// Получить приблизительный размер кэша в байтах
    pub fn cache_size_bytes(&self) -> usize {
        self.pages.len() * 4096
    }
}

impl Default for PageCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Глобальный page cache, защищенный Mutex'ом
pub static PAGE_CACHE: spin::Mutex<PageCache> = spin::Mutex::new(PageCache::new());

/// Вспомогательная функция для получения/загрузки страницы
pub fn page_cache_get_or_load<F>(
    inode: InodeId,
    page_idx: u64,
    loader: F,
) -> Arc<Mutex<CachedPage>>
where
    F: FnOnce() -> [u8; 4096],
{
    let mut cache = PAGE_CACHE.lock();
    cache.get_or_load(inode, page_idx, loader)
}

/// Вспомогательная функция для вытеснения страницы
pub fn page_cache_evict(inode: InodeId, page_idx: u64) -> bool {
    let mut cache = PAGE_CACHE.lock();
    cache.evict(inode, page_idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_cache_basic() {
        let mut cache = PageCache::new();
        
        let data = [42u8; 4096];
        let page = CachedPage::new(data);
        cache.insert(1, 0, page);
        
        assert_eq!(cache.cached_pages(), 1);
        assert!(cache.get(1, 0).is_some());
        assert!(cache.get(1, 1).is_none());
    }

    #[test]
    fn test_page_cache_dirty() {
        let mut cache = PageCache::new();
        
        let page = CachedPage::new([0u8; 4096]);
        cache.insert(1, 0, page);
        
        assert_eq!(cache.dirty_pages_count(), 0);
        assert!(cache.mark_dirty(1, 0));
        assert_eq!(cache.dirty_pages_count(), 1);
        assert!(cache.mark_clean(1, 0));
        assert_eq!(cache.dirty_pages_count(), 0);
    }

    #[test]
    fn test_page_cache_evict_inode() {
        let mut cache = PageCache::new();
        
        let page1 = CachedPage::new([1u8; 4096]);
        let page2 = CachedPage::new([2u8; 4096]);
        let page3 = CachedPage::new([3u8; 4096]);
        
        cache.insert(1, 0, page1);
        cache.insert(1, 1, page2);
        cache.insert(2, 0, page3);
        
        assert_eq!(cache.cached_pages(), 3);
        assert_eq!(cache.evict_inode(1), 2);
        assert_eq!(cache.cached_pages(), 1);
        assert!(cache.get(2, 0).is_some());
    }
}
