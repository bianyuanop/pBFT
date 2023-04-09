// module for report actions to a central server for visualizationinvent

use sqlx::MySqlConnection;

pub async fn report_message(conn: &mut MySqlConnection, action: String, origin: u128, target: u128, round: u128, state: String) {
    let raw_query = format!("
        INSERT INTO edges (action, origin, target, ts, round, state) 
        VALUES ('{0}', '{1}', '{2}', now(6), '{3}', '{4}')", action, origin, target, round, state);
    let query = sqlx::query(&raw_query);
    query.execute(conn).await.unwrap();
}


#[cfg(test)]
mod tests {
    use sqlx::{Connection, Executor};

    #[actix_rt::test]
    async fn insert() {
        let mut conn = sqlx::MySqlConnection::connect("mysql://chan:Diy.2002@localhost/pbft").await.unwrap();

        let action = "PrePrepare";
        let origin = "1";
        let target = "2";
        let round = "10";
        let state = "20";

        // test insert
        let raw_query = format!("
            INSERT INTO edges (action, origin, target, ts, round, state) 
            VALUES ('{0}', '{1}', '{2}', now(6), '{3}', '{4}')", action, origin, target, round, state);
        let query = sqlx::query(&raw_query);
        query.execute(&mut conn).await.unwrap();

        // then delete
        let delete_raw_query = format!("DELETE FROM edges");
        let delete_query = sqlx::query(&delete_raw_query);
        delete_query.execute(&mut conn).await.unwrap();
    }
}