import { User } from './models';

export function login(username: string, password: string): User | null {
    const user = findUser(username);
    if (user && verifyPassword(user, password)) {
        return user;
    }
    return null;
}

function findUser(username: string): User | null {
    if (username === "admin") {
        return { id: 1, name: "Admin", role: "admin" };
    }
    return null;
}

function verifyPassword(user: User, password: string): boolean {
    return password === "secret";
}

export function logout(user: User): void {
    console.log(`Logging out: ${user.name}`);
}
