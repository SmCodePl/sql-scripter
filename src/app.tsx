import { useCallback, useEffect, useState } from "react";
import { Database02 } from "@untitledui/icons";
import { AlertBanner } from "@/components/app/alert-banner";
import { FeaturedIcon } from "@/components/foundations/featured-icon/featured-icon";
import { AboutDialog } from "@/features/app/about-dialog";
import { SettingsDialog } from "@/features/app/settings-dialog";
import { ConfirmDeleteDialog } from "@/features/connections/confirm-delete-dialog";
import { ConnectionDialog } from "@/features/connections/connection-dialog";
import { ConnectionSidebar } from "@/features/connections/connection-sidebar";
import { ConnectionTab } from "@/features/workspace/connection-tab";
import { type PasswordPromptState, PasswordPromptDialog } from "@/features/workspace/password-prompt-dialog";
import { TabBar } from "@/features/workspace/tab-bar";
import { useI18n } from "@/i18n";
import { type AppInfo, type Connection, type ServerInfo, api, errorMessage } from "@/lib/api";

interface OpenTab {
    connectionId: number;
    info: ServerInfo;
}

/** Backend sends `"DECRYPT_FAILED:<id>"` when the stored password can't be decrypted. */
const parseDecryptError = (e: unknown): number | null => {
    const msg = errorMessage(e);
    const m = msg.match(/^DECRYPT_FAILED:(\d+)$/);
    return m ? Number(m[1]) : null;
};

export const App = ({ appInfo }: { appInfo: AppInfo }) => {
    const { t } = useI18n();
    const [settingsOpen, setSettingsOpen] = useState(false);
    const [aboutOpen, setAboutOpen] = useState(false);
    const [notice, setNotice] = useState<string | null>(
        appInfo.migratedConnections > 0 ? t("ws.migrated", { n: appInfo.migratedConnections }) : null,
    );
    const [connections, setConnections] = useState<Connection[]>([]);
    const [tabs, setTabs] = useState<OpenTab[]>([]);
    const [activeId, setActiveId] = useState<number | null>(null);
    const [connectingId, setConnectingId] = useState<number | null>(null);
    const [error, setError] = useState<string | null>(null);

    const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
    const [editing, setEditing] = useState<{ open: boolean; connection: Connection | null }>({ open: false, connection: null });
    const [deleting, setDeleting] = useState<Connection | null>(null);

    // Password re-entry prompt (when decryption key changed)
    const [passwordPrompt, setPasswordPrompt] = useState<PasswordPromptState | null>(null);
    const [passwordLoading, setPasswordLoading] = useState(false);

    const reload = useCallback(async () => {
        try {
            setConnections(await api.listConnections());
        } catch (e) {
            setError(errorMessage(e));
        }
    }, []);

    useEffect(() => {
        void reload();
    }, [reload]);

    const closeTab = useCallback(async (connectionId: number) => {
        setTabs((prev) => {
            const idx = prev.findIndex((t) => t.connectionId === connectionId);
            const next = prev.filter((t) => t.connectionId !== connectionId);
            setActiveId((active) => (active === connectionId ? (next[Math.max(0, idx - 1)]?.connectionId ?? null) : active));
            return next;
        });
        await api.closeSession(connectionId).catch(() => undefined);
    }, []);

    const addTab = (connectionId: number, info: ServerInfo) => {
        setTabs((prev) => (prev.some((t) => t.connectionId === connectionId) ? prev : [...prev, { connectionId, info }]));
        setActiveId(connectionId);
    };

    const connect = async (c: Connection) => {
        if (tabs.some((t) => t.connectionId === c.id)) {
            setActiveId(c.id);
            return;
        }
        setConnectingId(c.id);
        setError(null);
        try {
            const info = await api.openSession(c.id);
            addTab(c.id, info);
        } catch (e) {
            const failedId = parseDecryptError(e);
            if (failedId !== null) {
                // Password can't be decrypted — ask user to re-enter it.
                setPasswordPrompt({ connectionId: failedId, connectionName: c.name });
            } else {
                setError(`${c.name}: ${errorMessage(e)}`);
            }
        } finally {
            setConnectingId(null);
        }
    };

    const handlePasswordSubmit = async (connectionId: number, password: string) => {
        setPasswordLoading(true);
        setError(null);
        try {
            const info = await api.openSessionWithPassword(connectionId, password);
            setPasswordPrompt(null);
            addTab(connectionId, info);
        } catch (e) {
            setError(`${passwordPrompt?.connectionName ?? ""}: ${errorMessage(e)}`);
            setPasswordPrompt(null);
        } finally {
            setPasswordLoading(false);
        }
    };

    const byId = new Map(connections.map((c) => [c.id, c]));
    const openIds = new Set(tabs.map((t) => t.connectionId));

    return (
        <div className="flex h-full bg-secondary">
            <ConnectionSidebar
                connections={connections}
                openIds={openIds}
                activeId={activeId}
                connectingId={connectingId}
                collapsed={sidebarCollapsed}
                onToggleCollapse={() => setSidebarCollapsed((v) => !v)}
                onAdd={() => setEditing({ open: true, connection: null })}
                onEdit={(c) => setEditing({ open: true, connection: c })}
                onDelete={setDeleting}
                onConnect={connect}
                onSettings={() => setSettingsOpen(true)}
                onAbout={() => setAboutOpen(true)}
            />

            <main className="flex min-w-0 flex-1 flex-col">
                {tabs.length > 0 && (
                    <TabBar
                        tabs={tabs.map((t) => ({
                            id: t.connectionId,
                            label: byId.get(t.connectionId)?.name ?? "?",
                            sublabel: `${t.info.serverName} / ${t.info.database}`,
                        }))}
                        activeId={activeId}
                        onSelect={setActiveId}
                        onClose={closeTab}
                    />
                )}

                {notice && (
                    <div className="px-6 pt-4">
                        <AlertBanner tone="success" title={notice} onDismiss={() => setNotice(null)} />
                    </div>
                )}
                {error && (
                    <div className="px-6 pt-4">
                        <AlertBanner tone="error" title={t("ws.connectFailed")} onDismiss={() => setError(null)}>
                            {error}
                        </AlertBanner>
                    </div>
                )}

                <div className="relative min-h-0 flex-1">
                    {tabs.length === 0 && (
                        <div className="flex h-full flex-col items-center justify-center gap-4 text-center">
                            <FeaturedIcon icon={Database02} color="brand" theme="light" size="xl" />
                            <div>
                                <p className="text-lg font-semibold text-primary">{t("ws.pickTitle")}</p>
                                <p className="mt-1 max-w-md text-sm text-tertiary">{t("ws.pickBody")}</p>
                            </div>
                        </div>
                    )}

                    {tabs.map((t) => {
                        const connection = byId.get(t.connectionId);
                        if (!connection) return null;
                        return (
                            <div key={t.connectionId} role="tabpanel" hidden={t.connectionId !== activeId} className="absolute inset-0">
                                <ConnectionTab connection={connection} info={t.info} onExported={reload} />
                            </div>
                        );
                    })}
                </div>
            </main>

            <ConnectionDialog
                isOpen={editing.open}
                connection={editing.connection}
                onClose={() => setEditing({ open: false, connection: null })}
                onSaved={async (saved) => {
                    const wasEdit = editing.connection !== null;
                    setEditing({ open: false, connection: null });
                    if (wasEdit && openIds.has(saved.id)) await closeTab(saved.id);
                    await reload();
                }}
            />

            <SettingsDialog isOpen={settingsOpen} dbPath={appInfo.dbPath} onClose={() => setSettingsOpen(false)} />
            <AboutDialog isOpen={aboutOpen} version={appInfo.version} onClose={() => setAboutOpen(false)} />

            <ConfirmDeleteDialog
                connection={deleting}
                onCancel={() => setDeleting(null)}
                onConfirm={async (c) => {
                    try {
                        await api.deleteConnection(c.id);
                        if (openIds.has(c.id)) await closeTab(c.id);
                        await reload();
                    } catch (e) {
                        setError(errorMessage(e));
                    } finally {
                        setDeleting(null);
                    }
                }}
            />

            <PasswordPromptDialog
                prompt={passwordPrompt}
                isLoading={passwordLoading}
                onSubmit={handlePasswordSubmit}
                onClose={() => setPasswordPrompt(null)}
            />
        </div>
    );
};
