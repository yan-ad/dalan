import java.sql.*; import java.util.*; import java.util.logging.*; import java.lang.reflect.*;
public class FixtureDriver implements Driver {
 static Object proxy(Class<?> k,InvocationHandler h){return Proxy.newProxyInstance(FixtureDriver.class.getClassLoader(),new Class[]{k},h);}
 public Connection connect(String url,Properties props) throws SQLException {
  if(!"secret".equals(props.getProperty("password")))throw new SQLException("secret must not leak", "28000");
  System.out.println("driver stdout secret"); System.err.println("driver stderr secret");
  DatabaseMetaData md=(DatabaseMetaData)proxy(DatabaseMetaData.class,(o,m,a)-> {
   if(m.getName().equals("getDatabaseProductVersion"))return "fixture-17";
   if(m.getName().equals("getCatalogs"))return proxy(ResultSet.class,(x,n,b)-> {if(n.getName().equals("next"))return false;return null;});
   throw new SQLFeatureNotSupportedException();
  });
  return (Connection)proxy(Connection.class,(o,m,a)->switch(m.getName()){
   case "setReadOnly","setAutoCommit","rollback","close" -> null;
   case "isReadOnly" -> true; case "getAutoCommit" -> false; case "getMetaData" -> md;
   default -> throw new SQLFeatureNotSupportedException();
  });
 }
 public boolean acceptsURL(String u){return true;} public DriverPropertyInfo[] getPropertyInfo(String u,Properties p){return new DriverPropertyInfo[0];}
 public int getMajorVersion(){return 1;} public int getMinorVersion(){return 0;} public boolean jdbcCompliant(){return false;} public Logger getParentLogger(){return Logger.getGlobal();}
}
