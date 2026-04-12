export interface User {
    id: number;
    name: string;
    role: string;
}

export interface Session {
    user: User;
    token: string;
}

export enum AuthError {
    InvalidCredentials = "INVALID_CREDENTIALS",
    UserNotFound = "USER_NOT_FOUND",
    SessionExpired = "SESSION_EXPIRED",
}
