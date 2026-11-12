import { Database01, XClose } from "@untitledui/icons";
import { Button as AriaButton } from "react-aria-components";
import { useI18n } from "@/i18n";
import { cx } from "@/utils/cx";

export interface TabItem {
    id: number;
    label: string;
    sublabel: string;
}

interface TabBarProps {
    tabs: TabItem[];
    activeId: number | null;
    onSelect: (id: number) => void;
    onClose: (id: number) => void;
}

/** Closable tab strip styled like Untitled UI "underline" tabs. */
export const TabBar = ({ tabs, activeId, onSelect, onClose }: TabBarProps) => {
    const { t } = useI18n();
    return (
    <div role="tablist" aria-label={t("ws.tabs")} className="flex shrink-0 gap-1 overflow-x-auto border-b border-secondary bg-primary px-4 scrollbar-hide">
        {tabs.map((tab) => {
            const active = tab.id === activeId;
            return (
                <div
                    key={tab.id}
                    role="tab"
                    aria-selected={active}
                    tabIndex={active ? 0 : -1}
                    onClick={() => onSelect(tab.id)}
                    onAuxClick={(e) => e.button === 1 && onClose(tab.id)}
                    onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && onSelect(tab.id)}
                    className={cx(
                        "group relative -mb-px flex max-w-64 cursor-pointer items-center gap-2 border-b-2 px-3 py-3 outline-focus-ring focus-visible:outline-2",
                        active ? "border-smcode-lime text-primary" : "border-transparent text-quaternary hover:text-secondary",
                    )}
                    title={tab.sublabel}
                >
                    <Database01 className="size-4 shrink-0" />
                    <span className="truncate text-sm font-semibold">{tab.label}</span>
                    <AriaButton
                        aria-label={t("ws.closeTab", { name: tab.label })}
                        onPress={() => onClose(tab.id)}
                        className="rounded p-0.5 text-fg-quaternary opacity-60 outline-focus-ring group-hover:opacity-100 hover:bg-primary_hover hover:text-fg-quaternary_hover"
                    >
                        <XClose className="size-3.5" />
                    </AriaButton>
                </div>
            );
        })}
    </div>
    );
};
