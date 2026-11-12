import { invoke } from "@tauri-apps/api/core";

export type ObjectKind = "procedure" | "function" | "view" | "table";

export const OBJECT_KINDS = ["procedure", "function", "view", "table"] as const satisfies readonly ObjectKind[];

export type AuthKind = "sql" | "windows";

/** Structured connection fields; the connection string is derived from them by the backend. */
export interface ConnectionParams {
    server: string;
    port: number | null;
    database: string | null;
    auth: AuthKind;
    username: string | null;
    encrypt: boolean;
    trustServerCertificate: boolean;
}

export interface Connection extends ConnectionParams {
    id: number;
    name: string;
    hasPassword: boolean;
    /** Read-only, password masked. */
    connectionString: string;
    lastExportDir: string | null;
    createdAt: string;
    updatedAt: string;
}

export interface ConnectionInput extends ConnectionParams {
    name: string;
    /** `null` when editing = keep the stored password; `""` = remove it. */
    password: string | null;
}

export interface ServerInfo {
    serverName: string;
    database: string;
    productVersion: string;
    majorVersion: number;
}

export interface SearchFilter {
    kinds: ObjectKind[];
    modifiedFrom: string | null;
    modifiedTo: string | null;
    nameContains: string | null;
}

export interface DbObject {
    objectId: number;
    schemaName: string;
    objectName: string;
    kind: ObjectKind;
    typeCode: string;
    typeDesc: string;
    createDate: string;
    modifyDate: string;
    isModified: boolean;
}

export interface ObjectRef {
    schemaName: string;
    objectName: string;
    kind: ObjectKind;
}

export interface ExportReport {
    directory: string;
    written: string[];
    failed: { object: string; error: string }[];
}

export interface AppInfo {
    dbPath: string;
    dbCreated: boolean;
    migratedConnections: number;
    version: string;
    language: "pl" | "en" | null;
}

/** Backend errors arrive as plain strings. */
export const errorMessage = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e));

export const objectKey = (o: { schemaName: string; objectName: string }) => `${o.schemaName}.${o.objectName}`;

export const api = {
    appInfo: () => invoke<AppInfo>("app_info"),
    setLanguage: (language: "pl" | "en") => invoke<void>("set_language", { language }),

    listConnections: () => invoke<Connection[]>("list_connections"),
    createConnection: (input: ConnectionInput) => invoke<Connection>("create_connection", { input }),
    updateConnection: (id: number, input: ConnectionInput) => invoke<Connection>("update_connection", { id, input }),
    deleteConnection: (id: number) => invoke<void>("delete_connection", { id }),
    previewConnectionString: (params: ConnectionParams, hasPassword: boolean) =>
        invoke<string>("preview_connection_string", { params, hasPassword }),
    testConnection: (params: ConnectionParams, password: string | null, connectionId: number | null) =>
        invoke<ServerInfo>("test_connection", { params, password, connectionId }),

    openSession: (connectionId: number) => invoke<ServerInfo>("open_session", { connectionId }),
    openSessionWithPassword: (connectionId: number, password: string) =>
        invoke<ServerInfo>("open_session_with_password", { connectionId, password }),
    closeSession: (connectionId: number) => invoke<void>("close_session", { connectionId }),

    searchObjects: (connectionId: number, filter: SearchFilter) => invoke<DbObject[]>("search_objects", { connectionId, filter }),
    previewScript: (connectionId: number, object: ObjectRef) => invoke<string>("preview_script", { connectionId, object }),
    exportObjects: (connectionId: number, objects: ObjectRef[], directory: string) =>
        invoke<ExportReport>("export_objects", { connectionId, objects, directory }),
};
