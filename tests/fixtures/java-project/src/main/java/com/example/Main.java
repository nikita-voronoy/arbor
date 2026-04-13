package com.example;

public class Main {
    public static void main(String[] args) {
        AuthService auth = new AuthService();
        User user = auth.login("admin", "secret");
        if (user != null) {
            System.out.println("Welcome, " + user.getName());
            auth.logout(user);
        }
    }
}
