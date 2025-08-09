CREATE INDEX points_prediction_pred_id ON points(points_info, points_info ->> '$.Prediction[0]');
CREATE INDEX points_prediction_id ON points(points_info, points_info ->> '$.Prediction[1]');
CREATE INDEX points_created_at ON points(created_at);
CREATE INDEX predictions_prediction_id ON predictions(prediction_id);