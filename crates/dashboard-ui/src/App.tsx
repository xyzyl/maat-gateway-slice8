import { AuthProvider, useAuth } from "./lib/auth";
import { LoginPage } from "./pages/LoginPage";
import { Shell } from "./components/Shell";

export function App() {
  return (
    <AuthProvider>
      <AppRouter />
    </AuthProvider>
  );
}

function AppRouter() {
  const { state } = useAuth();

  if (state.status === "bootstrapping") {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="h-2 w-2 rounded-full bg-accent-500 animate-pulse" />
      </div>
    );
  }

  if (state.status === "anonymous") {
    return <LoginPage />;
  }

  return <Shell />;
}
