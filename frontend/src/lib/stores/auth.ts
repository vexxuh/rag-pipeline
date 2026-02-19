import { writable } from 'svelte/store';
import type { AuthState, User } from '$types/index';

const STORAGE_KEY = 'auth_token';

function createAuthStore() {
	const initial: AuthState = {
		user: null,
		token: typeof window !== 'undefined' ? localStorage.getItem(STORAGE_KEY) : null
	};

	const { subscribe, set, update } = writable<AuthState>(initial);

	return {
		subscribe,
		login(user: User, token: string) {
			if (typeof window !== 'undefined') {
				localStorage.setItem(STORAGE_KEY, token);
			}
			set({ user, token });
		},
		logout() {
			if (typeof window !== 'undefined') {
				localStorage.removeItem(STORAGE_KEY);
			}
			set({ user: null, token: null });
		},
		setUser(user: User) {
			update((state) => ({ ...state, user }));
		}
	};
}

export const authStore = createAuthStore();
