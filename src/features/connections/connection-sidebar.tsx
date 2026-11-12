import { useMemo, useState } from "react";
import { Database02, Edit01, LogIn04, Plus, SearchLg, Settings01, Trash01 } from "@untitledui/icons";
import { Button as AriaButton } from "react-aria-components";
import { Button } from "@/components/base/buttons/button";
import { ButtonUtility } from "@/components/base/buttons/button-utility";
import { Input } from "@/components/base/input/input";
import { useI18n } from "@/i18n";
import type { Connection } from "@/lib/api";
import { cx } from "@/utils/cx";
import smcodeLogo from "@/assets/smcode-logo.png";

interface ConnectionSidebarProps {
    connections: Connection[];
    openIds: Set<number>;
    activeId: number | null;
    connectingId: number | null;
    onAdd: () => void;
    onEdit: (c: Connection) => void;
    onDelete: (c: Connection) => void;
    onConnect: (c: Connection) => void;
    onSettings: () => void;
    onAbout: () => void;
}

const serverLabel = (c: Connection) => `${c.server}${c.port ? `,${c.port}` : ""}`;

export const ConnectionSidebar = ({
    connections,
    openIds,
    activeId,
    connectingId,
    onAdd,
    onEdit,
    onDelete,
    onConnect,
    onSettings,
    onAbout,
}: ConnectionSidebarProps) => {
    const { t } = useI18n();
    const [query, setQuery] = useState("");

    const filtered = useMemo(() => {
        const q = query.trim().toLowerCase();
        if (!q) return connections;
        return connections.filter((c) => [c.name, c.server, c.database ?? "", c.username ?? ""].some((v) => v.toLowerCase().includes(q)));
    }, [connections, query]);

    return (
        <aside className="flex h-full w-80 shrink-0 flex-col border-r border-secondary bg-primary">
            <div className="flex items-center gap-2.5 px-4 pt-5 pb-4">
                <div className="flex size-9 items-center justify-center rounded-lg bg-smcode-black">
                    <Database02 className="size-5 text-smcode-lime" />
                </div>
                <div className="leading-tight">
                    <p className="text-md font-semibold text-primary">SQL Scripter</p>
                    <p className="text-xs text-tertiary">{t("sidebar.byline")}</p>
                </div>
            </div>

            <div className="flex flex-col gap-3 border-b border-secondary px-4 pb-4">
                <Input size="sm" icon={SearchLg} placeholder={t("sidebar.search")} value={query} onChange={setQuery} aria-label={t("sidebar.search")} />
                <Button size="sm" color="primary" iconLeading={Plus} onClick={onAdd}>
                    {t("sidebar.add")}
                </Button>
            </div>

            <ul className="flex-1 overflow-y-auto px-2 py-2" aria-label={t("sidebar.list")}>
                {filtered.length === 0 && (
                    <li className="px-3 py-8 text-center text-sm text-tertiary">{connections.length === 0 ? t("sidebar.empty") : t("sidebar.noResults")}</li>
                )}
                {filtered.map((c) => {
                    const isOpen = openIds.has(c.id);
                    const isActive = activeId === c.id;
                    return (
                        <li
                            key={c.id}
                            onDoubleClick={() => onConnect(c)}
                            className={cx(
                                "group flex items-center gap-2 rounded-lg px-3 py-2.5 transition-colors",
                                isActive ? "bg-brand-primary" : "hover:bg-primary_hover",
                            )}
                        >
                            <span
                                aria-hidden
                                title={isOpen ? t("sidebar.connected") : undefined}
                                className={cx("size-2 shrink-0 rounded-full", isOpen ? "bg-smcode-lime" : "bg-quaternary")}
                            />
                            <div className="min-w-0 flex-1">
                                <p className="truncate text-sm font-semibold text-secondary">{c.name}</p>
                                <p className="truncate text-xs text-tertiary" title={c.connectionString}>
                                    {serverLabel(c)}
                                    {c.database && <> · {c.database}</>}
                                </p>
                            </div>
                            <div className="hidden items-center gap-0.5 group-focus-within:flex group-hover:flex">
                                <ButtonUtility size="xs" color="tertiary" icon={Edit01} tooltip={t("common.edit")} onClick={() => onEdit(c)} />
                                <ButtonUtility size="xs" color="tertiary" icon={Trash01} tooltip={t("common.delete")} onClick={() => onDelete(c)} />
                            </div>
                            <Button
                                size="xs"
                                color={isOpen ? "secondary" : "primary"}
                                iconLeading={LogIn04}
                                isLoading={connectingId === c.id}
                                isDisabled={connectingId !== null && connectingId !== c.id}
                                onClick={() => onConnect(c)}
                            >
                                {isOpen ? t("sidebar.open") : t("sidebar.connect")}
                            </Button>
                        </li>
                    );
                })}
            </ul>

            <div className="flex items-center gap-2 border-t border-secondary px-3 py-3">
                <AriaButton
                    onPress={onAbout}
                    aria-label={t("sidebar.about")}
                    className="flex flex-1 items-center rounded-md p-1 outline-focus-ring transition hover:bg-primary_hover focus-visible:outline-2"
                >
                    <img src={smcodeLogo} alt="SmCode" className="h-7 w-auto" draggable={false} />
                </AriaButton>
                <ButtonUtility size="sm" color="tertiary" icon={Settings01} tooltip={t("sidebar.settings")} onClick={onSettings} />
            </div>
        </aside>
    );
};
