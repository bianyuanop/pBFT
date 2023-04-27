import pymysql.cursors

conn = pymysql.connect(host='localhost',
                       user='chan',
                       password='Diy.2002',
                       database='pbft',
                       cursorclass=pymysql.cursors.DictCursor)

with conn:
    with conn.cursor() as cursor:
        sql = "DELETE FROM edges"
        cursor.execute(sql);
        conn.commit()



