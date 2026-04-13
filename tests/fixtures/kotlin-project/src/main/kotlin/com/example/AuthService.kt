package com.example

class AuthService : AuthProvider {
    override fun authenticate(username: String, password: String): User? {
        val user = findUser(username)
        return if (user != null && verifyPassword(user, password)) user else null
    }

    override fun revoke(session: Session) {
        println("Session revoked: ${session.token}")
    }

    fun login(username: String, password: String): User? {
        return authenticate(username, password)
    }

    fun logout(user: User) {
        println("User ${user.name} logged out")
    }

    private fun findUser(username: String): User? {
        return User(username, "$username@example.com", Role.ADMIN)
    }

    internal fun verifyPassword(user: User, password: String): Boolean {
        return password.isNotEmpty()
    }

    companion object {
        fun create(): AuthService = AuthService()
    }
}
