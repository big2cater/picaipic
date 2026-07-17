import { Ref } from 'vue';

/**
 * Represents a folder in an album's folder tree
 */
export interface Folder {
    id: number;
    name: string;
    path: string;
    created_at?: number;
    modified_at?: number;
    is_expanded?: boolean;
    is_favorite?: boolean;
    is_excluded_from_search?: boolean;
    children?: Folder[];
}

/**
 * Represents an album with its folder hierarchy
 */
export interface Album {
    id: number;
    name: string;
    path: string;
    description?: string;
    cover_file_id?: number;
    last_scan_time?: number;
    last_scan_count?: number;
    is_expanded?: boolean;
    is_favorite?: boolean;
    total?: number;
    indexed?: number;
    created_at?: number;
    modified_at?: number;
    children?: Folder[];
    is_accessible?: boolean;
}

/**
 * Selection context provided to album/folder components via inject
 */
export interface AlbumSelectionContext {
    // Current selection state
    albumId: Ref<number>;
    folderId: Ref<number | null>;
    folderPath: Ref<string>;
    selected: Ref<boolean>;  // true = album selected, false = folder selected

    // Selection actions
    selectAlbum: (album: Album) => void;
    selectFolder: (albumId: number, folder: Folder) => Promise<void>;

    // For navigating to a specific folder path (e.g., after folder move)
    expandAndSelectFolder: (albumId: number, folderPath: string) => Promise<void>;

    // For resetting selection (e.g., show all files)
    resetSelection: () => void;
}

/**
 * Injection key for album selection context
 */
export const ALBUM_SELECTION_KEY = Symbol('albumSelection');

/**
 * Represents a face bounding box
 */
export interface BBox {
    x: number;
    y: number;
    width: number;
    height: number;
}

/**
 * Represents a face record from the API/DB (raw data)
 */
export interface RawFace {
    id: number;
    file_id: number;
    person_id: number;
    person_name?: string;
    bbox: string; // JSON string
    created_at?: number;
    modified_at?: number;
}

/**
 * Represents a face record with parsed bounding box
 */
export interface Face extends Omit<RawFace, 'bbox'> {
    bbox: BBox;
}

/**
 * Paired video info for Live Photo / Motion Photo preview.
 * Returned by `getPairedVideo(fileId)`.
 */
export interface PairedVideoInfo {
    /** File path of the paired video (Apple) or source image (Motion Photo) */
    file_path: string;
    /** File ID of the paired file (Apple Live Photo only) */
    file_id?: number | null;
    /**
     * Type of the still being previewed:
     * 1=Apple Live Photo still, 3=Motion Photo, 4=HEIC-internal video.
     * Type 2 (Apple companion MOV) is not returned by getPairedVideo.
     */
    live_photo_type: number;
    /** For Motion Photo: embedded video byte offset */
    motion_video_offset?: number | null;
    /** For Motion Photo: embedded video length */
    motion_video_length?: number | null;
}

/** Options for `exportLivePhoto` (camelCase matches Rust serde). */
export interface ExportLivePhotoOptions {
    conflict?: 'keep_both' | 'replace';
    videoFormat?: string | null;
    stillFormat?: string | null;
    keyframeSec?: number | null;
    stampContentId?: boolean | null;
}

/** Result of `exportLivePhoto`. */
export interface ExportLivePhotoResult {
    outputs: string[];
    content_id?: string | null;
}
