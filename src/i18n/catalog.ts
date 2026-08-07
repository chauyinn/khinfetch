import type { LanguagePreference } from "../api/generated";
import { strings as en } from "./en";
import { strings as zhCN, type Messages } from "./zh-CN";

const catalogs: Record<LanguagePreference, Messages> = {
  "zh-cn": zhCN,
  en,
};

export const getStrings = (language: LanguagePreference): Messages => catalogs[language];
