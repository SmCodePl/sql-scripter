import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Eye } from "@untitledui/icons";
import type { Selection, SortDescriptor } from "react-aria-components";
import { Table } from "@/components/application/table/table";
import { Badge } from "@/components/base/badges/badges";
import type { BadgeColors } from "@/components/base/badges/badge-types";
import { ButtonUtility } from "@/components/base/buttons/button-utility";
import { useI18n } from "@/i18n";
import { type DbObject, type ObjectKind, objectKey } from "@/lib/api";

const kindColor: Record<ObjectKind, BadgeColors> = {
    procedure: "brand",
    function: "indigo",
    view: "sky",
    table: "orange",
};

type SortColumn = "kind" | "schemaName" | "objectName" | "createDate" | "modifyDate";

const PAGE_SIZES = [20, 100, 200, 500] as const;

interface ResultsTableProps {
    items: DbObject[];
    selectedKeys: Selection;
    onSelectionChange: (keys: Selection) => void;
    onPreview: (item: DbObject) => void;
    previewingKey: string | null;
}

export const ResultsTable = ({ items, selectedKeys, onSelectionChange, onPreview, previewingKey }: ResultsTableProps) => {
    const { t } = useI18n();
    const [sort, setSort] = useState<SortDescriptor>({ column: "modifyDate", direction: "descending" });
    const [pageSize, setPageSize] = useState<number>(100);
    const [page, setPage] = useState(0);

    // Context menu state
    const [contextMenu, setContextMenu] = useState<{ x: number; y: number; item: DbObject } | null>(null);
    const menuRef = useRef<HTMLDivElement>(null);

    // Reset page when items or pageSize change
    useEffect(() => setPage(0), [items, pageSize]);

    // Close context menu on click outside or Escape
    useEffect(() => {
        if (!contextMenu) return;
        const handleClick = (e: MouseEvent) => {
            if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
                setContextMenu(null);
            }
        };
        const handleKey = (e: KeyboardEvent) => {
            if (e.key === "Escape") setContextMenu(null);
        };
        document.addEventListener("mousedown", handleClick);
        document.addEventListener("keydown", handleKey);
        return () => {
            document.removeEventListener("mousedown", handleClick);
            document.removeEventListener("keydown", handleKey);
        };
    }, [contextMenu]);

    const sorted = useMemo(() => {
        const column = sort.column as SortColumn;
        const dir = sort.direction === "descending" ? -1 : 1;
        return [...items].sort((a, b) => String(a[column]).localeCompare(String(b[column]), "pl") * dir);
    }, [items, sort]);

    const totalPages = Math.max(1, Math.ceil(sorted.length / pageSize));
    const paged = useMemo(() => sorted.slice(page * pageSize, (page + 1) * pageSize), [sorted, page, pageSize]);

    const handleContextMenu = useCallback(
        (e: React.MouseEvent, item: DbObject) => {
            e.preventDefault();
            setContextMenu({ x: e.clientX, y: e.clientY, item });
        },
        [],
    );

    const handleContextPreview = useCallback(() => {
        if (contextMenu) {
            onPreview(contextMenu.item);
            setContextMenu(null);
        }
    }, [contextMenu, onPreview]);

    const Spinner = () => (
        <svg className="size-4 animate-spin text-fg-brand-primary" viewBox="0 0 32 32" fill="none">
            <circle cx="16" cy="16" r="14" stroke="currentColor" strokeWidth="4" strokeDashoffset="40" strokeDasharray="100" strokeLinecap="round" />
        </svg>
    );

    return (
        <div className="flex h-full flex-col">
            <div className="min-h-0 flex-1 overflow-auto">
                <Table
                    aria-label={t("results.aria")}
                    size="sm"
                    selectionMode="multiple"
                    selectionBehavior="toggle"
                    selectedKeys={selectedKeys}
                    onSelectionChange={onSelectionChange}
                    sortDescriptor={sort}
                    onSortChange={setSort}
                    wrapperClassName="h-full overflow-auto"
                >
                    <Table.Header className="sticky top-0 z-10">
                        <Table.Head id="kind" label={t("col.type")} allowsSorting />
                        <Table.Head id="schemaName" label={t("col.schema")} allowsSorting />
                        <Table.Head id="objectName" label={t("col.name")} allowsSorting isRowHeader />
                        <Table.Head id="status" label={t("col.status")} />
                        <Table.Head id="createDate" label={t("col.created")} allowsSorting />
                        <Table.Head id="modifyDate" label={t("col.modified")} allowsSorting />
                        <Table.Head id="actions" />
                    </Table.Header>
                    <Table.Body items={paged} dependencies={[t, previewingKey]}>
                        {(item) => {
                            const key = objectKey(item);
                            const isLoading = previewingKey === key;
                            return (
                                <Table.Row id={key} className="h-11" onContextMenu={(e: React.MouseEvent) => handleContextMenu(e, item)}>
                                    <Table.Cell>
                                        <Badge type="pill-color" size="sm" color={kindColor[item.kind]}>
                                            {t(`kind.${item.kind}`)}
                                        </Badge>
                                    </Table.Cell>
                                    <Table.Cell className="whitespace-nowrap">{item.schemaName}</Table.Cell>
                                    <Table.Cell className="w-full font-medium whitespace-nowrap text-primary">{item.objectName}</Table.Cell>
                                    <Table.Cell>
                                        {item.isModified ? (
                                            <Badge type="color" size="sm" color="warning">
                                                {t("status.modified")}
                                            </Badge>
                                        ) : (
                                            <Badge type="color" size="sm" color="success">
                                                {t("status.new")}
                                            </Badge>
                                        )}
                                    </Table.Cell>
                                    <Table.Cell className="whitespace-nowrap">
                                        <span title={item.createDate}>{item.createDate.slice(0, 16)}</span>
                                    </Table.Cell>
                                    <Table.Cell className="whitespace-nowrap">
                                        <span title={item.modifyDate}>{item.modifyDate.slice(0, 16)}</span>
                                    </Table.Cell>
                                    <Table.Cell className="px-3">
                                        {isLoading ? (
                                            <div className="flex items-center justify-center p-1.5">
                                                <Spinner />
                                            </div>
                                        ) : (
                                            <ButtonUtility size="xs" color="tertiary" icon={Eye} tooltip={t("preview.tooltip")} onClick={() => onPreview(item)} />
                                        )}
                                    </Table.Cell>
                                </Table.Row>
                            );
                        }}
                    </Table.Body>
                </Table>
            </div>

            {/* Pagination */}
            <div className="flex items-center justify-between border-t border-secondary px-5 py-2">
                <div className="flex items-center gap-2">
                    <span className="text-xs text-tertiary">{t("pagination.rowsPerPage")}</span>
                    <select
                        value={pageSize}
                        onChange={(e) => setPageSize(Number(e.target.value))}
                        className="rounded-md border border-secondary bg-primary px-2 py-1 text-xs text-primary outline-none focus:ring-2 focus:ring-focus-ring"
                    >
                        {PAGE_SIZES.map((s) => (
                            <option key={s} value={s}>
                                {s}
                            </option>
                        ))}
                    </select>
                </div>
                <div className="flex items-center gap-2">
                    <span className="text-xs text-tertiary">{t("pagination.page", { a: page + 1, b: totalPages })}</span>
                    <ButtonUtility
                        size="xs"
                        color="tertiary"
                        icon={ChevronLeft}
                        tooltip={t("pagination.prev")}
                        isDisabled={page === 0}
                        onClick={() => setPage((p) => p - 1)}
                    />
                    <ButtonUtility
                        size="xs"
                        color="tertiary"
                        icon={ChevronRight}
                        tooltip={t("pagination.next")}
                        isDisabled={page >= totalPages - 1}
                        onClick={() => setPage((p) => p + 1)}
                    />
                </div>
            </div>

            {/* Context menu */}
            {contextMenu && (
                <div
                    ref={menuRef}
                    style={{ position: "fixed", top: contextMenu.y, left: contextMenu.x, zIndex: 50 }}
                    className="min-w-44 rounded-lg border border-secondary bg-primary py-1 shadow-lg"
                >
                    <button
                        type="button"
                        className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-secondary hover:bg-secondary"
                        onClick={handleContextPreview}
                    >
                        <Eye className="size-4" />
                        {t("preview.contextMenu")}
                    </button>
                </div>
            )}
        </div>
    );
};
