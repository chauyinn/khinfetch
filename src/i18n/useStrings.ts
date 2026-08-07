import { useAppStore } from "../stores/useAppStore";
import { getStrings } from "./catalog";

export const useStrings = () => useAppStore((state) => getStrings(state.languageSetting));
