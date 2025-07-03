// Temporary type declarations for caller-utils
declare module '../../../target/ui/caller-utils' {
  export interface FileInfo {
    name: string;
    path: string;
    size: number;
    created: number;
    modified: number;
    is_directory: boolean;
    permissions: string;
  }

  export enum AuthScheme {
    Public = "public",
    Private = "private"
  }
}