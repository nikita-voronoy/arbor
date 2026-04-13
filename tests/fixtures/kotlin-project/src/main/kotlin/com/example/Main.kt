package com.example

object AppConfig {
    const val MAX_RETRIES = 3
    fun getTimeout(): Int = 30
}

fun main() {
    val auth = AuthService.create()
    val user = auth.login("admin", "secret")
    if (user != null) {
        println("Welcome, ${user.name}!")
        auth.logout(user)
    }
}

fun String.isValidEmail(): Boolean {
    return this.contains("@")
}
