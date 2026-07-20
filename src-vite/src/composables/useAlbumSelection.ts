import { provide, inject, computed, Ref } from 'vue';
import { libConfig } from '@/common/config';
import { selectFolder as apiSelectFolder } from '@/common/api';
import { Album, Folder, AlbumSelectionContext, ALBUM_SELECTION_KEY } from '@/common/types';

export type SelectionSource = 'album' | 'destFolder';

/**
 * Creates and provides an album selection context.
 * Call this in the root component (AlbumList) to provide selection state to all descendants.
 * 
 * @param source - 'album' for main album view, 'destFolder' for MoveTo dialog
 * @param onExpandAndSelect - callback to expand album and select folder (implemented by AlbumList)
 */
export function useAlbumSelectionProvider(
    source: SelectionSource,
    onExpandAndSelect?: (albumId: number, folderPath: string) => Promise<void>
) {
    const markAlbumActivated = () => {
        if (source !== 'album') return;
        libConfig.album.activateTick = Number(libConfig.album.activateTick || 0) + 1;
    };

    // Create refs that stay in sync with libConfig
    const albumId = computed({
        get: () => source === 'album' ? (libConfig.album.id ?? 0) : (libConfig.destFolder.albumId ?? 0),
        set: (val: number) => {
            if (source === 'album') {
                libConfig.album.id = val;
            } else {
                libConfig.destFolder.albumId = val;
            }
        }
    });

    const folderId = computed({
        get: () => source === 'album' ? libConfig.album.folderId : libConfig.destFolder.folderId,
        set: (val: number | null) => {
            if (source === 'album') {
                libConfig.album.folderId = val;
            } else {
                libConfig.destFolder.folderId = val;
            }
        }
    });

    const folderPath = computed({
        get: () => source === 'album' ? (libConfig.album.folderPath ?? '') : (libConfig.destFolder.folderPath ?? ''),
        set: (val: string) => {
            if (source === 'album') {
                libConfig.album.folderPath = val;
            } else {
                libConfig.destFolder.folderPath = val;
            }
        }
    });

    const selected = computed({
        get: () => source === 'album' ? (libConfig.album.selected ?? false) : (libConfig.destFolder.selected ?? false),
        set: (val: boolean) => {
            if (source === 'album') {
                libConfig.album.selected = val;
            } else {
                libConfig.destFolder.selected = val;
            }
        }
    });

    const clearLibraryQuickEntry = () => {
        if (source !== 'album') return;
        if (!libConfig.library) {
            (libConfig as any).library = { item: 'all-files' };
        }
        // Album/folder browsing leaves Favorites / On this day quick entries
        libConfig.library.item = 'all-files';
    };

    /**
     * Select an album (shows all files in the album)
     */
    const selectAlbum = (album: Album) => {
        clearLibraryQuickEntry();
        albumId.value = album.id;
        folderPath.value = album.path;
        selected.value = true;
        markAlbumActivated();
    };

    /**
     * Select a folder within an album
     */
    const selectFolder = async (albumIdVal: number, folder: Folder) => {
        clearLibraryQuickEntry();
        markAlbumActivated();
        const selectedPath = folder.path;
        // Folder-tree nodes are filesystem nodes and often lack the DB folder id.
        // When AlbumList remounts and restores the already selected folder, keep
        // the known id instead of null → resolve, which reloads Content twice.
        const isRestoringCurrentFolder =
            albumId.value === albumIdVal &&
            folderPath.value === selectedPath &&
            !selected.value;
        albumId.value = albumIdVal;
        if (!isRestoringCurrentFolder) {
            folderId.value = Number(folder.id || 0) || null;
        }
        folderPath.value = selectedPath;
        selected.value = false;

        const result = await apiSelectFolder(albumIdVal, selectedPath);
        if (result) {
            // Only apply if this is still the active selection target.
            if (albumId.value === albumIdVal && folderPath.value === selectedPath) {
                albumId.value = albumIdVal;
                folderId.value = result.id;
                folderPath.value = result.path;
                selected.value = false;
            }
        }
    };

    /**
     * Expand the album tree to a specific folder and select it
     * Used after folder move operations
     */
    const expandAndSelectFolder = async (albumIdVal: number, targetFolderPath: string) => {
        if (onExpandAndSelect) {
            await onExpandAndSelect(albumIdVal, targetFolderPath);
        }
    };

    /**
     * Reset selection to show all files
     */
    const resetSelection = () => {
        albumId.value = 0;
        folderId.value = null;
        folderPath.value = '';
        selected.value = false;
    };

    // Create the context object
    const context: AlbumSelectionContext = {
        albumId: albumId as unknown as Ref<number>,
        folderId: folderId as unknown as Ref<number | null>,
        folderPath: folderPath as unknown as Ref<string>,
        selected: selected as unknown as Ref<boolean>,
        selectAlbum,
        selectFolder,
        expandAndSelectFolder,
        resetSelection,
    };

    // Provide the context to all descendants
    provide(ALBUM_SELECTION_KEY, context);

    return context;
}

/**
 * Injects the album selection context.
 * Call this in child components (AlbumFolder) to access the selection state.
 */
export function useAlbumSelection(): AlbumSelectionContext {
    const context = inject<AlbumSelectionContext>(ALBUM_SELECTION_KEY);
    if (!context) {
        throw new Error('useAlbumSelection must be used within a component that provides AlbumSelectionContext');
    }
    return context;
}
