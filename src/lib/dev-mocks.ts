// Fake backend for `npm run dev` in a plain browser (outside Tauri), to iterate on the UI quickly.
// Never bundled into the app: main.tsx imports it only in DEV when Tauri internals are absent.
import { mockIPC } from "@tauri-apps/api/mocks";
import type { Connection, ConnectionParams, DbObject } from "./api";

const now = "2026-11-11T09:00:00Z";
let nextId = 4;
let language: string | null = null;

const conn = (id: number, name: string, p: Partial<ConnectionParams>, hasPassword = true): Connection => {
    const params: ConnectionParams = { server: "", port: null, database: null, auth: "sql", username: "sa", encrypt: true, trustServerCertificate: false, ...p };
    return { id, name, ...params, hasPassword, connectionString: display(params, hasPassword), lastExportDir: null, createdAt: now, updatedAt: now };
};

// Mirrors ConnectionParams::display_string in Rust (mock only).
const display = (p: ConnectionParams, hasPassword: boolean) =>
    [
        `Server=${p.server}${p.port ? `,${p.port}` : ""}`,
        p.database ? `Database=${p.database}` : null,
        p.auth === "windows" ? "Integrated Security=true" : `User Id=${p.username ?? ""};Password=${hasPassword ? "********" : ""}`,
        `Encrypt=${p.encrypt}`,
        `TrustServerCertificate=${p.trustServerCertificate}`,
    ]
        .filter(Boolean)
        .join(";");

const connections: Connection[] = [
    conn(1, "PROD – Sprzedaż", { server: "sqlprod01", port: 1433, database: "Sales", username: "app" }),
    conn(2, "TEST – Logistyka", { server: "sqltest\\SQLEXPRESS", database: "Handling", auth: "windows", username: null }, false),
    conn(3, "Lokalny Docker", { server: "localhost", port: 1433, database: "Dev", trustServerCertificate: true }),
];

const objects: DbObject[] = [
    ["handling", "usp_TransportOrder_Save", "procedure", "P", true, "2026-11-11 14:52:10"],
    ["handling", "transport_orders_journal", "table", "U", true, "2026-11-11 13:20:44"],
    ["dbo", "fn_GetOrderTotal", "function", "FN", true, "2026-11-11 11:03:00"],
    ["dbo", "vw_OpenOrders", "view", "V", false, "2026-11-11 10:41:31"],
    ["dbo", "MessageHistory", "table", "U", true, "2026-11-11 09:15:02"],
    ["report", "usp_DailySummary", "procedure", "P", false, "2026-11-11 08:02:55"],
].map(([schemaName, objectName, kind, typeCode, isModified, modifyDate], i) => ({
    objectId: 1000 + i,
    schemaName: schemaName as string,
    objectName: objectName as string,
    kind: kind as DbObject["kind"],
    typeCode: typeCode as string,
    typeDesc: String(kind).toUpperCase(),
    createDate: isModified ? "2025-03-02 10:00:00" : (modifyDate as string),
    modifyDate: modifyDate as string,
    isModified: isModified as boolean,
}));

const info = (id: number) => ({ serverName: id === 1 ? "SQLPROD01" : "SQLTEST", database: id === 1 ? "Sales" : "Handling", productVersion: "16.0.4135.4", majorVersion: 16 });

export const installDevMocks = () => {
    mockIPC(async (cmd, args) => {
        const a = (args ?? {}) as Record<string, any>;
        await new Promise((r) => setTimeout(r, 250));
        switch (cmd) {
            case "app_info":
                return {
                    dbPath: "~/Library/Application Support/com.sm-code.sqlscripter/connections.db3",
                    dbCreated: false,
                    migratedConnections: 0,
                    version: "0.2.0",
                    language,
                };
            case "set_language":
                language = a.language;
                return null;
            case "preview_connection_string":
                return display(a.params, a.hasPassword);
            case "list_connections":
                return [...connections].sort((x, y) => x.name.localeCompare(y.name));
            case "create_connection": {
                const { name, password, ...params } = a.input;
                const c = conn(nextId++, name, params, Boolean(password));
                connections.push(c);
                return c;
            }
            case "update_connection": {
                const idx = connections.findIndex((x) => x.id === a.id);
                const { name, password, ...params } = a.input;
                const hasPassword = password === null ? connections[idx]!.hasPassword : password !== "";
                connections[idx] = conn(a.id, name, params, hasPassword);
                return connections[idx];
            }
            case "delete_connection":
                connections.splice(connections.findIndex((x) => x.id === a.id), 1);
                return null;
            case "test_connection":
                return info(1);
            case "open_session":
                if (a.connectionId === 3) throw "Błąd SQL Server: Login failed for user 'sa'.";
                return info(a.connectionId);
            case "close_session":
                return null;
            case "search_objects":
                return objects.filter((o) => a.filter.kinds.includes(o.kind));
            case "preview_script":
                return `-- ${a.object.kind}: [${a.object.schemaName}].[${a.object.objectName}]\r\nSET ANSI_NULLS ON\r\nGO\r\nCREATE OR ALTER PROCEDURE ...\r\nGO\r\n`;
            case "export_objects":
                return { directory: "/Users/me/sql", written: a.objects.map((o: any) => `${o.schemaName}.${o.objectName}.sql`), failed: [] };
            case "plugin:dialog|open":
                return "/Users/me/sql";
            default:
                throw `mock: unknown command ${cmd}`;
        }
    });
};
