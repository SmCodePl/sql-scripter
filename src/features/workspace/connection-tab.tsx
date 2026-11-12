import { useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Download01, SearchLg, SearchRefraction } from "@untitledui/icons";
import type { Selection } from "react-aria-components";
import { LoadingIndicator } from "@/components/application/loading-indicator/loading-indicator";
import { AlertBanner } from "@/components/app/alert-banner";
import { Badge } from "@/components/base/badges/badges";
import { Button } from "@/components/base/buttons/button";
import { Checkbox } from "@/components/base/checkbox/checkbox";
import { Input } from "@/components/base/input/input";
import { FeaturedIcon } from "@/components/foundations/featured-icon/featured-icon";
import { useI18n } from "@/i18n";
import { type Connection, type DbObject, type ExportReport, OBJECT_KINDS, type ObjectKind, type ServerInfo, api, errorMessage, objectKey } from "@/lib/api";
import { addDays, today } from "@/lib/utils";
import { ResultsTable } from "./results-table";
import { type PreviewState, ScriptPreviewDialog } from "./script-preview-dialog";

interface ConnectionTabProps {
    connection: Connection;
    info: ServerInfo;
    onExported: () => void;
}

const ALL_KINDS: ObjectKind[] = [...OBJECT_KINDS];

const presets = [
    { key: "filter.today", days: 0 },
    { key: "filter.7d", days: 6 },
    { key: "filter.30d", days: 29 },
] as const;

const kindFilterKey = {
    procedure: "filter.procedures",
    function: "filter.functions",
    view: "filter.views",
    table: "filter.tables",
} as const;

export const ConnectionTab = ({ connection, info, onExported }: ConnectionTabProps) => {
    const { t } = useI18n();
    const [kinds, setKinds] = useState<Set<ObjectKind>>(new Set(ALL_KINDS));
    const [dateFrom, setDateFrom] = useState(today());
    const [dateTo, setDateTo] = useState(today());
    const [name, setName] = useState("");

    const [items, setItems] = useState<DbObject[] | null>(null);
    const [selected, setSelected] = useState<Selection>(new Set());
    const [searching, setSearching] = useState(false);
    const [exporting, setExporting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [report, setReport] = useState<ExportReport | null>(null);
    const [preview, setPreview] = useState<PreviewState | null>(null);
    const [previewingKey, setPreviewingKey] = useState<string | null>(null);

    const allKinds = kinds.size === ALL_KINDS.length;

    const toggleKind = (kind: ObjectKind, on: boolean) =>
        setKinds((prev) => {
            const next = new Set(prev);
            if (on) next.add(kind);
            else next.delete(kind);
            return next;
        });

    const selectedObjects = useMemo(() => {
        if (!items) return [];
        if (selected === "all") return items;
        return items.filter((i) => selected.has(objectKey(i)));
    }, [items, selected]);

    const search = async () => {
        setSearching(true);
        setError(null);
        setReport(null);
        try {
            const result = await api.searchObjects(connection.id, {
                kinds: [...kinds],
                modifiedFrom: dateFrom ? `${dateFrom}T00:00:00` : null,
                // Upper bound is exclusive: the day after "to".
                modifiedTo: dateTo ? `${addDays(dateTo, 1)}T00:00:00` : null,
                nameContains: name.trim() || null,
            });
            setItems(result);
            setSelected(new Set());
        } catch (e) {
            setError(errorMessage(e));
        } finally {
            setSearching(false);
        }
    };

    const exportSelected = async () => {
        const directory = await openDialog({
            directory: true,
            multiple: false,
            title: t("results.chooseFolder"),
            defaultPath: connection.lastExportDir ?? undefined,
        });
        if (typeof directory !== "string") return;

        setExporting(true);
        setError(null);
        setReport(null);
        try {
            const refs = selectedObjects.map(({ schemaName, objectName, kind }) => ({ schemaName, objectName, kind }));
            setReport(await api.exportObjects(connection.id, refs, directory));
            onExported();
        } catch (e) {
            setError(errorMessage(e));
        } finally {
            setExporting(false);
        }
    };

    const showPreview = async (item: DbObject) => {
        const key = objectKey(item);
        const base = { title: `${item.schemaName}.${item.objectName}`, fileName: `${item.schemaName}.${item.objectName}.sql` };
        setPreviewingKey(key);
        setPreview({ ...base, script: null, error: null });
        try {
            const script = await api.previewScript(connection.id, item);
            setPreview({ ...base, script, error: null });
        } catch (e) {
            setPreview({ ...base, script: null, error: errorMessage(e) });
        } finally {
            setPreviewingKey(null);
        }
    };

    return (
        <div className="flex h-full min-h-0 flex-col gap-4 p-6">
            {/* Header */}
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                <h1 className="text-display-xs font-semibold text-primary">{connection.name}</h1>
                <Badge type="color" size="sm" color="gray">
                    {info.serverName} / {info.database}
                </Badge>
                <span className="text-sm text-tertiary">SQL Server {info.productVersion}</span>
            </div>

            {/* Filters */}
            <div className="flex flex-col gap-4 rounded-xl bg-primary p-5 shadow-xs ring-1 ring-secondary">
                <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
                    <span className="text-sm font-semibold text-secondary">{t("filter.export")}</span>
                    <Checkbox
                        label={t("filter.all")}
                        isSelected={allKinds}
                        isIndeterminate={kinds.size > 0 && !allKinds}
                        onChange={(on) => setKinds(on ? new Set(ALL_KINDS) : new Set())}
                    />
                    <span className="h-5 w-px bg-border-secondary" aria-hidden />
                    {OBJECT_KINDS.map((kind) => (
                        <Checkbox key={kind} label={t(kindFilterKey[kind])} isSelected={kinds.has(kind)} onChange={(on) => toggleKind(kind, on)} />
                    ))}
                </div>

                <div className="flex flex-wrap items-end gap-3">
                    <Input label={t("filter.from")} type="date" size="sm" value={dateFrom} onChange={setDateFrom} className="w-44" />
                    <Input label={t("filter.to")} type="date" size="sm" value={dateTo} onChange={setDateTo} className="w-44" />
                    <div className="flex gap-1 pb-0.5">
                        {presets.map((p) => (
                            <Button
                                key={p.key}
                                size="sm"
                                color="tertiary"
                                onClick={() => {
                                    setDateFrom(addDays(today(), -p.days));
                                    setDateTo(today());
                                }}
                            >
                                {t(p.key)}
                            </Button>
                        ))}
                        <Button
                            size="sm"
                            color="tertiary"
                            onClick={() => {
                                setDateFrom("");
                                setDateTo("");
                            }}
                        >
                            {t("filter.noDate")}
                        </Button>
                    </div>
                    <Input label={t("filter.name")} size="sm" icon={SearchLg} placeholder={t("filter.namePlaceholder")} value={name} onChange={setName} className="w-56" />
                    <div className="flex-1" />
                    <Button size="md" color="primary" iconLeading={SearchLg} onClick={search} isLoading={searching} isDisabled={kinds.size === 0}>
                        {t("filter.search")}
                    </Button>
                </div>
            </div>

            {error && (
                <AlertBanner tone="error" title={t("results.errorTitle")} onDismiss={() => setError(null)}>
                    {error}
                </AlertBanner>
            )}
            {report && (
                <AlertBanner
                    tone={report.failed.length ? "error" : "success"}
                    title={
                        report.failed.length
                            ? t("results.savedWithErrors", { n: report.written.length, e: report.failed.length })
                            : t("results.saved", { n: report.written.length })
                    }
                    onDismiss={() => setReport(null)}
                >
                    <p>{t("results.folder", { dir: report.directory })}</p>
                    {report.failed.length > 0 && (
                        <ul className="mt-2 list-disc pl-5">
                            {report.failed.map((f) => (
                                <li key={f.object}>
                                    <span className="font-medium">{f.object}</span>: {f.error}
                                </li>
                            ))}
                        </ul>
                    )}
                </AlertBanner>
            )}

            {/* Results */}
            <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl bg-primary shadow-xs ring-1 ring-secondary">
                <div className="flex items-center gap-3 border-b border-secondary px-5 py-3">
                    <h2 className="text-md font-semibold text-primary">{t("results.title")}</h2>
                    {items && (
                        <Badge type="modern" size="sm" color="gray">
                            {items.length}
                        </Badge>
                    )}
                    <div className="flex-1" />
                    {items && items.length > 0 && (
                        <span className="text-sm text-tertiary">
                            {t("results.selected", { a: selectedObjects.length, b: items.length })}
                        </span>
                    )}
                    <Button
                        size="sm"
                        color="primary"
                        iconLeading={Download01}
                        onClick={exportSelected}
                        isLoading={exporting}
                        isDisabled={selectedObjects.length === 0}
                    >
                        {t("results.export")}
                    </Button>
                </div>

                <div className="min-h-0 flex-1">
                    {searching && items === null ? (
                        <div className="flex h-full items-center justify-center">
                            <LoadingIndicator type="line-spinner" size="md" label={t("results.searching")} />
                        </div>
                    ) : items === null || items.length === 0 ? (
                        <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
                            <FeaturedIcon icon={SearchRefraction} color="gray" theme="modern" size="lg" />
                            <p className="text-md font-semibold text-primary">{items === null ? t("results.initialTitle") : t("results.emptyTitle")}</p>
                            <p className="max-w-sm text-sm text-tertiary">
                                {items === null ? t("results.initialBody") : t("results.emptyBody")}
                            </p>
                        </div>
                    ) : (
                        <ResultsTable items={items} selectedKeys={selected} onSelectionChange={setSelected} onPreview={showPreview} previewingKey={previewingKey} />
                    )}
                </div>
            </div>

            <ScriptPreviewDialog preview={preview} onClose={() => setPreview(null)} />
        </div>
    );
};
