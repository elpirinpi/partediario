import { create } from "zustand";
import type { ChatSessionSummary, Novedad } from "@parte/shared";

interface WorkspaceState {
  session: ChatSessionSummary | null;
  selectedId: number | null;
  filter: string;
  setSession: (session: ChatSessionSummary | null) => void;
  setNovedades: (novedades: Novedad[]) => void;
  setSelectedId: (id: number | null) => void;
  setFilter: (filter: string) => void;
  updateNovedad: (id: number, patch: Partial<Novedad>) => void;
  movePhotos: (fromId: number, toId: number) => void;
  reset: () => void;
}

export const useWorkspace = create<WorkspaceState>((set) => ({
  session: null,
  selectedId: null,
  filter: "",
  setSession: (session) =>
    set({
      session,
      selectedId: session?.novedades[0]?.id ?? null,
      filter: "",
    }),
  setNovedades: (novedades) =>
    set((state) => (state.session ? { session: { ...state.session, novedades } } : state)),
  setSelectedId: (selectedId) => set({ selectedId }),
  setFilter: (filter) => set({ filter }),
  updateNovedad: (id, patch) =>
    set((state) => {
      if (!state.session) return state;
      return {
        session: {
          ...state.session,
          novedades: state.session.novedades.map((n) => (n.id === id ? { ...n, ...patch } : n)),
        },
      };
    }),
  movePhotos: (fromId, toId) =>
    set((state) => {
      if (!state.session || fromId === toId) return state;
      const source = state.session.novedades.find((n) => n.id === fromId);
      const target = state.session.novedades.find((n) => n.id === toId);
      if (!source || !target || source.photos.length === 0) return state;
      const moved = source.photos;
      return {
        session: {
          ...state.session,
          novedades: state.session.novedades.map((n) => {
            if (n.id === target.id) {
              return { ...n, photos: [...n.photos, ...moved] };
            }
            if (n.id === source.id) {
              return {
                ...n,
                photos: [],
                include: n.bodyRaw.trim() || n.headers.length > 0 ? n.include : false,
              };
            }
            return n;
          }),
        },
      };
    }),
  reset: () => set({ session: null, selectedId: null, filter: "" }),
}));
