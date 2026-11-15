declare interface FileSystemHandle {
  kind: "file" | "directory";
  name: string;
}

declare interface FileSystemFileHandle extends FileSystemHandle {
  kind: "file";
  name: string;
}

declare interface ShowSaveFilePickerOptions {
  suggestedName?: string;
  types?: Array<{
    description: string;
    accept: Record<string, string[]>;
  }>;
}

declare interface Window {
  showSaveFilePicker?: (
    options?: ShowSaveFilePickerOptions,
  ) => Promise<FileSystemFileHandle>;
}

declare module "*.svelte" {
  const content: any;
  export default content;
}
