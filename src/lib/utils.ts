const pad = (n: number) => String(n).padStart(2, "0");

/** `yyyy-mm-dd` in local time. */
export const toDateInput = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

export const addDays = (date: string, days: number): string => {
    const [y, m, d] = date.split("-").map(Number);
    return toDateInput(new Date(y!, m! - 1, d! + days));
};

export const today = () => toDateInput(new Date());
