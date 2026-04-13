package com.example

data class User(val name: String, val email: String, val role: Role)

data class Session(val token: String, val expiresAt: Long)

sealed class AuthError {
    data class InvalidCredentials(val message: String) : AuthError()
    object UserNotFound : AuthError()
    object SessionExpired : AuthError()
}

enum class Role {
    ADMIN,
    EDITOR,
    VIEWER,
    GUEST
}

interface AuthProvider {
    fun authenticate(username: String, password: String): User?
    fun revoke(session: Session)
}
