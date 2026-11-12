import { type ReactNode, createContext, useCallback, useContext, useMemo, useState } from "react";
import { api } from "@/lib/api";
import { en } from "./en";
import { type TranslationKey, pl } from "./pl";

export type Language = "pl" | "en";

export const LANGUAGES: { code: Language; label: string }[] = [
    { code: "pl", label: "Polski" },
    { code: "en", label: "English" },
];

const dictionaries: Record<Language, Record<TranslationKey, string>> = { pl, en };

export type Translate = (key: TranslationKey, params?: Record<string, string | number>) => string;

interface I18nContextValue {
    language: Language;
    /** Changes the UI language and saves it in the `settings` table of connections.db3. */
    setLanguage: (language: Language) => Promise<void>;
    t: Translate;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export const systemLanguage = (): Language => (navigator.language.toLowerCase().startsWith("pl") ? "pl" : "en");

export const I18nProvider = ({ initial, children }: { initial: Language; children: ReactNode }) => {
    const [language, setLang] = useState<Language>(initial);

    const t = useCallback<Translate>(
        (key, params) => {
            let text = dictionaries[language][key] ?? key;
            if (params) {
                for (const [k, v] of Object.entries(params)) text = text.replaceAll(`{${k}}`, String(v));
            }
            return text;
        },
        [language],
    );

    const setLanguage = useCallback(async (next: Language) => {
        setLang(next);
        document.documentElement.lang = next;
        await api.setLanguage(next);
    }, []);

    const value = useMemo(() => ({ language, setLanguage, t }), [language, setLanguage, t]);
    return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
};

export const useI18n = () => {
    const ctx = useContext(I18nContext);
    if (!ctx) throw new Error("useI18n outside I18nProvider");
    return ctx;
};
