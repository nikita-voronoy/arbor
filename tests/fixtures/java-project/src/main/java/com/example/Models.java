package com.example;

public class User {
    private String name;
    private String email;
    private Role role;

    public User(String name, String email, Role role) {
        this.name = name;
        this.email = email;
        this.role = role;
    }

    public String getName() { return name; }
    public String getEmail() { return email; }
}

class Session {
    private String token;

    Session(String token) {
        this.token = token;
    }

    public String getToken() { return token; }
}

enum Role {
    ADMIN,
    EDITOR,
    VIEWER,
    GUEST
}

interface AuthProvider {
    User authenticate(String username, String password);
    void revoke(Session session);
}

record Credentials(String username, String password) {}
