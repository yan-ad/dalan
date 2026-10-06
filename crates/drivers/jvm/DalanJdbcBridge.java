import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.sql.*;
import java.util.*;
import java.util.jar.*;

/** Java 17 source-file launcher bridge. Drivers are trusted executable code, not sandboxed.
 * Neither driver-internal/wire allocations nor driver-created processes are contained here. */
public final class DalanJdbcBridge {
    static final int FRAME = 1024 * 1024, RESPONSE = 2 * 1024 * 1024, STR = 65536, CELL = 4096;
    static final int OBJECTS = 1000, COLS = 512;
    static class Failure extends Exception { final String category; Failure(String c) { category = c; } }
    static void require(boolean b, String c) throws Failure { if (!b) throw new Failure(c); }
    static Map<String,Object> obj(Object... a) { Map<String,Object> m = new LinkedHashMap<>(); for (int i=0;i<a.length;i+=2) m.put((String)a[i],a[i+1]); return m; }
    static String str(Map<String,Object> m, String k) throws Failure { Object v=m.get(k); require(v instanceof String,"request"); return (String)v; }
    static String optional(Map<String,Object> m, String k) throws Failure { return m.get(k)==null ? null : str(m,k); }
    static int num(Map<String,Object> m, String k, int def, int max) throws Failure { Object v=m.get(k); if(v==null)return def; require(v instanceof Long,"request"); long n=(Long)v; require(n>=0 && n<=max,"request"); return (int)n; }
    static String bounded(String s) throws Failure { if(s==null)return ""; require(s.length()<=STR,"limit"); return s; }
    public static void main(String[] args) {
        // Capture the response sink before suppressing all driver chatter (including stderr).
        PrintStream response = System.out;
        System.setOut(new PrintStream(OutputStream.nullOutputStream()));
        System.setErr(new PrintStream(OutputStream.nullOutputStream()));
        Object result;
        try {
            byte[] input=System.in.readNBytes(FRAME+1); require(input.length<=FRAME,"limit");
            String text=new String(input,StandardCharsets.UTF_8);
            Object parsed=new Json(text).parse(); require(parsed instanceof Map,"request");
            @SuppressWarnings("unchecked") Map<String,Object> request=(Map<String,Object>)parsed;
            result=run(request);
        } catch(Throwable e) { result=obj("ok",false,"errorcategory",category(e)); }
        try { response.write(encode(result)); response.flush(); }
        catch(Throwable e) { response.print("{\"ok\":false,\"errorcategory\":\"limit\"}"); response.flush(); }
    }
    static String category(Throwable e) {
        if(e instanceof Failure) return ((Failure)e).category;
        if(e instanceof SQLTimeoutException) return "timeout";
        if(e instanceof SQLFeatureNotSupportedException || e instanceof UnsupportedOperationException) return "unsupported";
        if(e instanceof SQLException) { String state=((SQLException)e).getSQLState(); if(state!=null && state.startsWith("28"))return "authentication"; }
        return "database";
    }
    static Object run(Map<String,Object> r) throws Exception {
        Object configured=r.get("jars"); require(configured instanceof List,"request");
        List<?> jars=(List<?>)configured; require(!jars.isEmpty() && jars.size()<=16,"request");
        URL[] urls=new URL[jars.size()];
        for(int i=0;i<jars.size();i++) {
            require(jars.get(i) instanceof String,"request"); Path p=Path.of((String)jars.get(i));
            require(p.isAbsolute() && !Files.isSymbolicLink(p),"request");
            // Manifest Class-Path would silently extend the configured allowlist.
            try(JarFile j=new JarFile(p.toFile())) { Manifest m=j.getManifest(); if(m!=null) require(m.getMainAttributes().getValue(Attributes.Name.CLASS_PATH)==null,"unsupported"); }
            urls[i]=p.toUri().toURL();
        }
        DriverManager.setLoginTimeout(num(r,"connect_timeout",10,60));
        try(URLClassLoader loader=new URLClassLoader(urls,ClassLoader.getPlatformClassLoader())) {
            Thread.currentThread().setContextClassLoader(loader);
            Object instance=Class.forName(str(r,"driver_class"),true,loader).getDeclaredConstructor().newInstance();
            require(instance instanceof Driver,"request");
            Properties props=new Properties(); if(r.get("user")!=null)props.setProperty("user",str(r,"user")); if(r.get("password")!=null)props.setProperty("password",str(r,"password"));
            Connection connected=((Driver)instance).connect(str(r,"url"),props); props.clear(); require(connected!=null,"unsupported");
            try(Connection c=connected) {
                c.setReadOnly(true); require(c.isReadOnly(),"unsupported");
                c.setAutoCommit(false); require(!c.getAutoCommit(),"unsupported");
                try { return operation(c,r); } finally { c.rollback(); }
            }
        }
    }
    // Catalog-less drivers expose one database named "". An explicit database
    // uses setCatalog when catalogs exist, otherwise setSchema; no silent fallback.
    static final String DEFAULT_DATABASE="(connection default)";
    static List<String> databases(Connection c) throws Exception {
        List<String> names=new ArrayList<>();
        try(ResultSet rs=c.getMetaData().getCatalogs()) { while(rs.next()) { require(names.size()<OBJECTS,"limit"); String s=bounded(rs.getString(1));require(!s.isEmpty()&&!s.equals(DEFAULT_DATABASE)&&s.getBytes(StandardCharsets.UTF_8).length<=256,"limit"); if(!names.contains(s))names.add(s); } }
        if(names.isEmpty())names.add(DEFAULT_DATABASE); return names;
    }
    static void select(Connection c,String db) throws Exception {
        if(db==null || db.isEmpty())return;
        List<String> names=databases(c);
        if(names.size()==1 && names.get(0).equals(DEFAULT_DATABASE)) {if(db.equals(DEFAULT_DATABASE))return;c.setSchema(db); require(db.equals(c.getSchema()),"unsupported");}
        else { require(names.contains(db),"request"); c.setCatalog(db); require(db.equals(c.getCatalog()),"unsupported"); }
    }
    static String quote(DatabaseMetaData md,String value) throws Exception {
        String q=md.getIdentifierQuoteString(); require(q!=null && !q.trim().isEmpty() && q.length()<=4,"unsupported");
        require(value!=null && !value.isEmpty() && value.length()<=STR,"limit");
        // Some JDBC drivers report '[' as their quote; closing bracket differs.
        String end=q.equals("[")?"]":q;
        return q+value.replace(end,end+end)+end;
    }
    record Table(String catalog,String schema,String name,String display,String kind) {}
    static List<Table> tableRecords(Connection c) throws Exception {
        DatabaseMetaData md=c.getMetaData(); List<Table> tables=new ArrayList<>(); Set<String> displays=new HashSet<>();
        String schema=c.getSchema();
        try(ResultSet rs=md.getTables(c.getCatalog(),null,"%",new String[]{"TABLE","VIEW"})) {
            while(rs.next()) {
                String cat=rs.getString("TABLE_CAT"), sch=rs.getString("TABLE_SCHEM"), name=bounded(rs.getString("TABLE_NAME")), type=rs.getString("TABLE_TYPE");
                if(c.getCatalog()!=null && cat!=null && !c.getCatalog().equals(cat))continue;
                // With no catalogs and an explicit schema selection, constrain discovery.
                if((c.getCatalog()==null || c.getCatalog().isEmpty()) && schema!=null && !schema.isEmpty() && !schema.equals(sch))continue;
                if(!"TABLE".equals(type) && !"VIEW".equals(type))continue;
                String display=(sch==null || sch.isEmpty()?"":quote(md,bounded(sch))+".")+quote(md,name);
                require(tables.size()<OBJECTS && displays.add(display),"limit");
                tables.add(new Table(cat,sch,name,display,"TABLE".equals(type)?"BASE TABLE":"VIEW"));
            }
        } return tables;
    }
    static List<Object> tableInfos(Connection c) throws Exception { List<Object> values=new ArrayList<>(); for(Table t:tableRecords(c))values.add(obj("name",t.display(),"kind",t.kind())); return values; }
    static Table resolve(Connection c,String name) throws Exception {
        Table found=null; for(Table t:tableRecords(c))if(t.display().equals(name)) { require(found==null,"request"); found=t; }
        require(found!=null,"request"); return found;
    }
    static String pattern(DatabaseMetaData md,String s) throws Exception {
        if(s==null)return null; String e=md.getSearchStringEscape(); require(e!=null && !e.isEmpty(),"unsupported");
        return s.replace(e,e+e).replace("%",e+"%").replace("_",e+"_");
    }
    static List<Object> columnInfos(Connection c,Table t) throws Exception {
        DatabaseMetaData md=c.getMetaData(); Set<String> pk=new HashSet<>();
        try(ResultSet rs=md.getPrimaryKeys(t.catalog(),t.schema(),t.name())) { int n=0; while(rs.next()) { require(++n<=COLS,"limit"); pk.add(bounded(rs.getString("COLUMN_NAME"))); } }
        List<Object> columns=new ArrayList<>();
        try(ResultSet rs=md.getColumns(t.catalog(),pattern(md,t.schema()),pattern(md,t.name()),"%")) {
            while(rs.next()) {
                if(!Objects.equals(t.catalog(),rs.getString("TABLE_CAT")) || !Objects.equals(t.schema(),rs.getString("TABLE_SCHEM")) || !t.name().equals(rs.getString("TABLE_NAME")))continue;
                require(columns.size()<COLS,"limit"); String name=bounded(rs.getString("COLUMN_NAME"));
                columns.add(obj("name",name,"data_type",bounded(rs.getString("TYPE_NAME")),"nullable",rs.getInt("NULLABLE")!=DatabaseMetaData.columnNoNulls,"is_primary_key",pk.contains(name)));
            }
        } return columns;
    }
    static Object operation(Connection c,Map<String,Object> r) throws Exception {
        String op=str(r,"op");
        if(op.equals("test"))return obj("ok",true,"report",obj("server_version",bounded(c.getMetaData().getDatabaseProductVersion()),"databases",databases(c)));
        if(op.equals("catalog")) {
            List<Object> values=new ArrayList<>(); int total=0;
            for(String db:databases(c)) { select(c,db); List<Object> tables=tableInfos(c); total+=tables.size(); require(total<=10000,"limit"); values.add(obj("name",db,"tables",tables)); }
            return obj("ok",true,"catalog",obj("databases",values));
        }
        select(c,optional(r,"database"));
        if(op.equals("tables"))return obj("ok",true,"tables",tableInfos(c));
        if(op.equals("columns"))return obj("ok",true,"columns",columnInfos(c,resolve(c,str(r,"table"))));
        if(op.equals("browse") || op.equals("query")) {
            int limit=num(r,"limit",100,200), offset=op.equals("browse")?num(r,"offset",0,10000):0; require(limit>0,"request");
            String sql;
            if(op.equals("browse")) { Table t=resolve(c,str(r,"table")); sql="SELECT * FROM "+t.display(); }
            else sql=str(r,"sql"); // Trusted parent must apply conservative AST validator.
            long start=System.nanoTime(); Object page;
            try(PreparedStatement st=c.prepareStatement(sql)) {
                st.setQueryTimeout(num(r,"timeout",20,120)); st.setMaxRows(offset+limit+1); st.setFetchSize(Math.min(limit+1,201));
                try(ResultSet rs=st.executeQuery()) { page=page(rs,limit,offset); }
            }
            if(op.equals("browse"))return obj("ok",true,"page",page);
            return obj("ok",true,"result",obj("page",page,"elapsed_ms",(System.nanoTime()-start)/1000000,"warnings",List.of()));
        }
        throw new Failure("request");
    }
    static Object cell(ResultSet rs,int index,int type,boolean[] truncated) throws Exception {
        boolean binary=type==Types.BINARY || type==Types.VARBINARY || type==Types.LONGVARBINARY || type==Types.BLOB;
        if(binary) {
            try(InputStream in=rs.getBinaryStream(index)) {
                if(in==null)return "Null"; byte[] bytes=in.readNBytes(CELL/2+1); if(bytes.length>CELL/2) { bytes=Arrays.copyOf(bytes,CELL/2); truncated[0]=true; }
                return obj("Binary",HexFormat.of().formatHex(bytes));
            }
        }
        String value;
        try(Reader reader=rs.getCharacterStream(index)) {
            if(reader==null)return "Null"; StringBuilder b=new StringBuilder(); char[] buf=new char[512]; int n;
            while(b.length()<=CELL && (n=reader.read(buf,0,Math.min(buf.length,CELL+1-b.length())))!=-1) { if(n==0)continue; b.append(buf,0,n); }
            if(b.length()>CELL) { b.setLength(CELL); truncated[0]=true; } value=b.toString();
            while(value.getBytes(StandardCharsets.UTF_8).length>CELL) {
                value=value.substring(0,value.offsetByCodePoints(value.length(),-1)); truncated[0]=true;
            }
        }
        String kind=switch(type) {
            case Types.BIT,Types.BOOLEAN,Types.TINYINT,Types.SMALLINT,Types.INTEGER,Types.BIGINT,Types.FLOAT,Types.REAL,Types.DOUBLE,Types.NUMERIC,Types.DECIMAL -> "Number";
            case Types.DATE,Types.TIME,Types.TIMESTAMP,Types.TIME_WITH_TIMEZONE,Types.TIMESTAMP_WITH_TIMEZONE -> "Temporal";
            default -> "Text";
        }; return obj(kind,value);
    }
    static Object page(ResultSet rs,int limit,int offset) throws Exception {
        ResultSetMetaData md=rs.getMetaData(); int count=md.getColumnCount(); require(count<=COLS,"limit");
        List<Object> columns=new ArrayList<>(); for(int i=1;i<=count;i++) columns.add(obj("name",bounded(md.getColumnLabel(i)),"data_type",bounded(md.getColumnTypeName(i)),"nullable",md.isNullable(i)!=ResultSetMetaData.columnNoNulls,"is_primary_key",false));
        List<Object> rows=new ArrayList<>(); boolean[] trunc={false}; boolean more=false; int skipped=0, budget=encode(columns).length;
        while(skipped<offset && rs.next())skipped++;
        while(rs.next()) {
            if(rows.size()>=limit) { more=true; break; }
            List<Object> row=new ArrayList<>(); for(int i=1;i<=count;i++)row.add(cell(rs,i,md.getColumnType(i),trunc));
            int cost=encode(row).length; if(budget+cost>RESPONSE-65536) { require(!rows.isEmpty(),"limit"); more=true; trunc[0]=true; break; }
            rows.add(row); budget+=cost+1;
        }
        return obj("columns",columns,"rows",rows,"has_more",more,"next_offset",more?(long)offset+rows.size():null,"offset",offset,"truncated",trunc[0]);
    }
    static byte[] encode(Object value) throws Exception {
        ByteArrayOutputStream out=new ByteArrayOutputStream(); write(value,out,0); return out.toByteArray();
    }
    static void put(ByteArrayOutputStream out,String s) throws Exception { byte[] b=s.getBytes(StandardCharsets.UTF_8); require(out.size()+b.length<=RESPONSE,"limit"); out.write(b); }
    static void write(Object v,ByteArrayOutputStream out,int depth) throws Exception {
        require(depth<=32,"limit");
        if(v==null) { put(out,"null"); return; }
        if(v instanceof String) {
            String s=(String)v; require(s.length()<=STR,"limit"); put(out,"\"");
            for(int i=0;i<s.length();i++) { char c=s.charAt(i); switch(c) {
                case '"': put(out,"\\\""); break; case '\\': put(out,"\\\\"); break;
                case '\n': put(out,"\\n"); break; case '\r': put(out,"\\r"); break; case '\t': put(out,"\\t"); break;
                default: if(c<32 || Character.isSurrogate(c))put(out,String.format("\\u%04x",(int)c)); else put(out,String.valueOf(c));
            } } put(out,"\""); return;
        }
        if(v instanceof Boolean || v instanceof Number) { put(out,v.toString()); return; }
        if(v instanceof Map) { put(out,"{"); boolean first=true; for(Object entry:((Map<?,?>)v).entrySet()) { Map.Entry<?,?> e=(Map.Entry<?,?>)entry; if(!first)put(out,","); first=false; write(e.getKey(),out,depth+1); put(out,":"); write(e.getValue(),out,depth+1); } put(out,"}"); return; }
        if(v instanceof List) { put(out,"["); boolean first=true; for(Object item:(List<?>)v) { if(!first)put(out,","); first=false; write(item,out,depth+1); } put(out,"]"); return; }
        throw new Failure("request");
    }
    /** Small strict JSON parser: frame, depth, token/string sizes and collection counts bounded. */
    static final class Json {
        final String s; int p=0, nodes=0; Json(String s) { this.s=s; }
        void ws() { while(p<s.length() && " \r\n\t".indexOf(s.charAt(p))>=0)p++; }
        Object parse() throws Exception { Object v=value(0); ws(); require(p==s.length(),"request"); return v; }
        char take() throws Exception { require(p<s.length(),"request"); return s.charAt(p++); }
        Object value(int depth) throws Exception {
            require(depth<=16 && ++nodes<=8192,"limit"); ws(); char c=take();
            if(c=='"')return string();
            if(c=='{') { Map<String,Object> m=new LinkedHashMap<>(); ws(); if(p<s.length() && s.charAt(p)=='}') { p++;return m; } while(true) { ws(); require(take()=='"',"request"); String k=string(); ws(); require(take()==':',"request"); require(!m.containsKey(k),"request"); m.put(k,value(depth+1)); ws(); c=take(); if(c=='}')return m; require(c==',',"request"); } }
            if(c=='[') { List<Object> a=new ArrayList<>(); ws(); if(p<s.length() && s.charAt(p)==']') { p++;return a; } while(true) { a.add(value(depth+1)); ws(); c=take(); if(c==']')return a; require(c==',',"request"); } }
            if(c=='t' || c=='f' || c=='n') { String suffix=c=='t'?"rue":c=='f'?"alse":"ull"; require(s.startsWith(suffix,p),"request"); p+=suffix.length(); return c=='n'?null:c=='t'; }
            int start=p-1; require(c=='-' || (c>='0' && c<='9'),"request"); while(p<s.length() && Character.isDigit(s.charAt(p)))p++; String n=s.substring(start,p); require(n.length()<=20 && n.matches("-?(0|[1-9][0-9]*)"),"request"); try{return Long.parseLong(n);}catch(NumberFormatException e){throw new Failure("request");}
        }
        String string() throws Exception {
            StringBuilder b=new StringBuilder(); while(true) { char c=take(); if(c=='"')return b.toString(); require(c>=32,"request"); if(c=='\\') { c=take(); switch(c) {
                case '"': case '\\': case '/': break; case 'b': c='\b';break; case 'f':c='\f';break; case 'n':c='\n';break; case 'r':c='\r';break; case 't':c='\t';break;
                case 'u': int n=0; for(int i=0;i<4;i++){int d=Character.digit(take(),16);require(d>=0,"request");n=n*16+d;}c=(char)n;break;
                default:throw new Failure("request");
            } } b.append(c); require(b.length()<=STR,"limit"); }
        }
    }
}
