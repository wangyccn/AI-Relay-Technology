import { useState, useCallback, useEffect, useMemo } from "react";
import { NavLink, Route, Routes, useLocation } from "react-router-dom";
import {
  Box,
  AppBar,
  Toolbar,
  Typography,
  Drawer,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  CssBaseline,
  Container,
  IconButton,
  Divider,
  useMediaQuery,
} from "@mui/material";
import {
  Menu as MenuIcon,
  Dashboard as DashboardIcon,
  Folder as FolderIcon,
  Build as BuildIcon,
  SmartToy as RobotIcon,
  Settings as SettingsIcon,
  ChevronLeft as ChevronLeftIcon,
  ChevronRight as ChevronRightIcon,
} from "@mui/icons-material";
import { ThemeProvider } from "@mui/material/styles";
import { buildMuiTheme, lightTheme } from "./theme";
import { api } from "./api";
import { ToastProvider, ModalProvider } from "./components";
import Dashboard from "./pages/Dashboard";
import Projects from "./pages/Projects";
import Tools from "./pages/Tools";
import Models from "./pages/Models";
import Settings from "./pages/Settings";
import {
  applyThemeFromConfig,
  normalizeThemeConfig,
  onThemeChange,
  resolveTheme,
  startThemeWatcher,
} from "./theme/runtime";

const fullDrawerWidth = 260;
const compactDrawerWidth = 84;

interface NavItem {
  to: string;
  label: string;
  icon: React.ReactElement;
}

const navItems: NavItem[] = [
  { to: "/", label: "总览", icon: <DashboardIcon /> },
  { to: "/projects", label: "项目", icon: <FolderIcon /> },
  { to: "/tools", label: "系统配置", icon: <BuildIcon /> },
  { to: "/models", label: "模型路由", icon: <RobotIcon /> },
  { to: "/settings", label: "设置", icon: <SettingsIcon /> },
];

function App() {
  const isMobile = useMediaQuery(lightTheme.breakpoints.down('sm'));
  const isCompact = useMediaQuery(lightTheme.breakpoints.down('md'));
  const [resolvedTheme, setResolvedTheme] = useState(() => resolveTheme(undefined));
  const muiTheme = useMemo(
    () => buildMuiTheme(resolvedTheme.theme, resolvedTheme.isDark),
    [resolvedTheme],
  );
  const [mobileOpen, setMobileOpen] = useState(false);
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);
  const isCollapsed = !isMobile && (isCompact || isSidebarCollapsed);
  const drawerWidth = isCollapsed ? compactDrawerWidth : fullDrawerWidth;
  const location = useLocation();

  const handleDrawerToggle = useCallback(() => {
    setMobileOpen(!mobileOpen);
  }, [mobileOpen]);

  const handleSidebarCollapse = useCallback(() => {
    setIsSidebarCollapsed((prev) => !prev);
  }, []);

  useEffect(() => {
    let alive = true;
    const stopWatcher = startThemeWatcher();
    const unsubscribe = onThemeChange((detail) => {
      setResolvedTheme(detail.resolved);
    });

    api.config
      .get()
      .then((cfg) => {
        if (!alive) return;
        applyThemeFromConfig(normalizeThemeConfig(cfg.theme));
      })
      .catch(() => {
        if (!alive) return;
        applyThemeFromConfig(undefined);
      });

    return () => {
      alive = false;
      stopWatcher();
      unsubscribe();
    };
  }, []);

  const renderDrawerContent = (collapsed: boolean, showBrand: boolean, showFooter: boolean) => (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {showBrand && (
        <>
          {/* Brand Section */}
          <Toolbar
            sx={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              px: 2,
              py: collapsed ? 2 : 3,
            }}
          >
            <Box sx={{ textAlign: 'center' }}>
              <Typography
                variant={collapsed ? "h5" : "h4"}
                component="div"
                sx={{
                  fontWeight: 800,
                  background: "linear-gradient(135deg, #0f6f63, #e68a2e)",
                  WebkitBackgroundClip: "text",
                  WebkitTextFillColor: "transparent",
                  letterSpacing: collapsed ? 1 : 2,
                }}
              >
                CCR
              </Typography>
              {!collapsed && (
                <Typography
                  variant="caption"
                  color="text.secondary"
                  sx={{ display: 'block', mt: 0.5, fontSize: '0.7rem' }}
                >
                  多模型控制中心
                </Typography>
              )}
            </Box>
          </Toolbar>

          <Divider />
        </>
      )}

      {/* Navigation */}
      <List sx={{ px: collapsed ? 1.5 : 2, py: 2, flex: 1 }}>
        {navItems.map((item) => (
          <ListItem
            key={item.to}
            disablePadding
            sx={{ mb: 0.5, px: collapsed ? 0.5 : 0 }}
          >
            <ListItemButton
              component={NavLink}
              to={item.to}
              end={item.to === "/"}
              selected={location.pathname === item.to}
              title={collapsed ? item.label : undefined}
              sx={{
                borderRadius: 2,
                py: 1.5,
                px: collapsed ? 1.5 : 2,
                minHeight: 48,
                justifyContent: collapsed ? 'center' : 'flex-start',
                width: '100%',
                mx: collapsed ? 0.5 : 0,
                "&.active": {
                  bgcolor: "primary.main",
                  color: "primary.contrastText",
                  "& .MuiSvgIcon-root": {
                    color: "inherit",
                  },
                  "&:hover": {
                    bgcolor: "primary.dark",
                  },
                },
                "&:hover": {
                  bgcolor: "action.hover",
                },
              }}
              onClick={() => {
                if (isMobile) {
                  setMobileOpen(false);
                }
              }}
            >
              <ListItemIcon
                sx={{
                  color: "inherit",
                  minWidth: collapsed ? 0 : 48,
                  mr: collapsed ? 0 : 1,
                  display: 'flex',
                  justifyContent: 'center',
                }}
              >
                {item.icon}
              </ListItemIcon>
              <ListItemText
                primary={item.label}
                sx={{
                  display: collapsed ? 'none' : 'block',
                  '& .MuiTypography-root': {
                    fontWeight: location.pathname === item.to ? 600 : 500,
                    fontSize: '0.875rem',
                  },
                }}
              />
            </ListItemButton>
          </ListItem>
        ))}
      </List>

      {showFooter && (
        <>
          <Divider />

          {/* Version */}
          {!collapsed && (
            <Box sx={{ px: 2, py: 2, textAlign: 'center' }}>
              <Typography variant="caption" color="text.secondary">
                v1.0.0
              </Typography>
            </Box>
          )}
        </>
      )}
    </Box>
  );

  const drawer = (
    <Box
      className={`sidebar-shell${isCollapsed ? " collapsed" : ""}`}
      sx={{ height: '100%', display: 'flex', flexDirection: 'column', position: 'relative' }}
    >
      {renderDrawerContent(isCollapsed, true, true)}
      {isCollapsed && (
        <Box className="sidebar-hover" sx={{ width: fullDrawerWidth }}>
          {renderDrawerContent(false, false, false)}
        </Box>
      )}
    </Box>
  );

  return (
    <ThemeProvider theme={muiTheme}>
      <ToastProvider>
        <ModalProvider>
        <Box sx={{ display: "flex", minHeight: '100vh' }}>
          <CssBaseline />

          {/* Top AppBar */}
          <AppBar
            position="fixed"
            elevation={0}
            sx={{
              width: { sm: `calc(100% - ${drawerWidth}px)` },
              ml: { sm: `${drawerWidth}px` },
              borderBottom: '1px solid',
              borderColor: 'divider',
              transition: 'margin 200ms ease, width 200ms ease',
            }}
          >
            <Toolbar>
              <IconButton
                color="inherit"
                aria-label="open drawer"
                edge="start"
                onClick={handleDrawerToggle}
                sx={{ mr: 2, display: { sm: "none" } }}
              >
                <MenuIcon />
              </IconButton>
              <IconButton
                color="inherit"
                aria-label={isCollapsed ? "expand sidebar" : "collapse sidebar"}
                onClick={handleSidebarCollapse}
                sx={{ mr: 1, display: { xs: "none", md: "none", lg: "inline-flex" } }}
              >
                {isCollapsed ? <ChevronRightIcon /> : <ChevronLeftIcon />}
              </IconButton>
              <Box sx={{ flex: 1 }}>
                <Typography variant="h6" noWrap component="div" sx={{ fontWeight: 700 }}>
                  CCR 控制台
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  统一管理多模型 CLI 与路由转发的可视化面板
                </Typography>
              </Box>
            </Toolbar>
          </AppBar>

          {/* Navigation Drawer */}
          <Box
            component="nav"
            sx={{
              width: { sm: drawerWidth },
              flexShrink: { sm: 0 },
              transition: 'width 200ms ease',
            }}
          >
            {/* Mobile drawer */}
            <Drawer
              variant="temporary"
              open={mobileOpen}
              onClose={handleDrawerToggle}
              ModalProps={{
                keepMounted: true,
              }}
              sx={{
                display: { xs: "block", sm: "none" },
                "& .MuiDrawer-paper": {
                  boxSizing: "border-box",
                  width: drawerWidth,
                  bgcolor: "background.paper",
                  borderTop: '1px solid',
                  borderColor: 'divider',
                  overflow: isCollapsed ? 'visible' : 'hidden',
                  overflowY: isCollapsed ? 'visible' : 'auto',
                },
              }}
            >
              {drawer}
            </Drawer>

            {/* Desktop drawer */}
            <Drawer
              variant="permanent"
              sx={{
                display: { xs: "none", sm: "block" },
                "& .MuiDrawer-paper": {
                  boxSizing: "border-box",
                  width: drawerWidth,
                  bgcolor: "background.paper",
                  borderRight: '1px solid',
                  borderColor: 'divider',
                  overflow: isCollapsed ? 'visible' : 'hidden',
                  overflowY: isCollapsed ? 'visible' : 'auto',
                  transition: 'width 200ms ease',
                },
              }}
              open
            >
              {drawer}
            </Drawer>
          </Box>

          {/* Main Content */}
          <Box
            component="main"
            sx={{
              flexGrow: 1,
              width: { sm: `calc(100% - ${drawerWidth}px)` },
              minHeight: "100vh",
              bgcolor: "background.default",
              display: 'flex',
              flexDirection: 'column',
              transition: 'margin 200ms ease, width 200ms ease',
            }}
          >
            <Toolbar />
            <Container
              maxWidth="xl"
              sx={{
                py: 3,
                flex: 1,
                px: { xs: 2, sm: 3 },
              }}
            >
              <Routes>
                <Route path="/" element={<Dashboard />} />
                <Route path="/projects" element={<Projects />} />
                <Route path="/tools" element={<Tools />} />
                <Route path="/models" element={<Models />} />
                <Route path="/settings" element={<Settings />} />
              </Routes>
            </Container>
          </Box>
        </Box>
        </ModalProvider>
      </ToastProvider>
    </ThemeProvider>
  );
}

export default App;
