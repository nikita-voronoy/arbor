package main

import "fmt"

type User struct {
	ID   int
	Name string
	Role string
}

type AuthError struct {
	Code    int
	Message string
}

type Authenticator interface {
	Login(username, password string) (*User, error)
	Logout(user *User)
}

func (e *AuthError) Error() string {
	return fmt.Sprintf("auth error %d: %s", e.Code, e.Message)
}

func NewUser(id int, name, role string) *User {
	return &User{ID: id, Name: name, Role: role}
}

func Login(username, password string) (*User, error) {
	user := FindUser(username)
	if user == nil {
		return nil, &AuthError{Code: 404, Message: "not found"}
	}
	if !VerifyPassword(user, password) {
		return nil, &AuthError{Code: 401, Message: "bad password"}
	}
	return user, nil
}

func FindUser(username string) *User {
	if username == "admin" {
		return NewUser(1, "Admin", "admin")
	}
	return nil
}

func VerifyPassword(user *User, password string) bool {
	_ = user
	return password == "secret"
}

func main() {
	user, err := Login("admin", "secret")
	if err != nil {
		fmt.Println("Error:", err)
		return
	}
	fmt.Println("Logged in:", user.Name)
}
