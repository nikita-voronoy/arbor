package com.example;

public class AuthService implements AuthProvider {
    @Override
    public User authenticate(String username, String password) {
        User user = findUser(username);
        if (user != null && verifyPassword(user, password)) {
            return user;
        }
        return null;
    }

    @Override
    public void revoke(Session session) {
        System.out.println("Session revoked: " + session.getToken());
    }

    public User login(String username, String password) {
        return authenticate(username, password);
    }

    public void logout(User user) {
        System.out.println("User logged out: " + user.getName());
    }

    private User findUser(String username) {
        return new User(username, username + "@example.com", Role.ADMIN);
    }

    protected boolean verifyPassword(User user, String password) {
        return password != null && !password.isEmpty();
    }
}
