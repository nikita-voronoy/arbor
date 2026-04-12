namespace MyApp
{
    public class User
    {
        public string Name { get; set; }
        public string Email { get; set; }
        public Role UserRole { get; set; }
    }

    public class Session
    {
        public string Token { get; set; }
        public DateTime ExpiresAt { get; set; }
    }

    public enum Role
    {
        Admin,
        Editor,
        Viewer,
        Guest
    }

    public interface IAuthProvider
    {
        User Authenticate(string username, string password);
        void Revoke(Session session);
    }
}
