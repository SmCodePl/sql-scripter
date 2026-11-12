import { useMemo, useState } from "react";
import { Eye } from "@untitledui/icons";
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

interface ResultsTableProps {
    items: DbObject[];
    selectedKeys: Selection;
    onSelectionChange: (keys: Selection) => void;
    onPreview: (item: DbObject) => void;
}

export const ResultsTable = ({ items, selectedKeys, onSelectionChange, onPreview }: ResultsTableProps) => {
    const { t } = useI18n();
    const [sort, setSort] = useState<SortDescriptor>({ column: "modifyDate", direction: "descending" });

    const sorted = useMemo(() => {
        const column = sort.column as SortColumn;
        const dir = sort.direction === "descending" ? -1 : 1;
        return [...items].sort((a, b) => String(a[column]).localeCompare(String(b[column]), "pl") * dir);
    }, [items, sort]);

    return (
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
            <Table.Body items={sorted} dependencies={[t]}>
                {(item) => (
                    <Table.Row id={objectKey(item)} className="h-11">
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
                            <ButtonUtility size="xs" color="tertiary" icon={Eye} tooltip={t("preview.tooltip")} onClick={() => onPreview(item)} />
                        </Table.Cell>
                    </Table.Row>
                )}
            </Table.Body>
        </Table>
    );
};
